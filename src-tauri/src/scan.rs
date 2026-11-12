//! Salt-okunur tarama motoru. Hiçbir şeyi silmez; yalnızca `safety::Policy`
//! onayından geçen adayları boyutlarıyla listeler.
//!
//! Hız: dizin yürüyüşü ve boyut hesabı rayon ile paraleldir. `DirEntry::metadata`
//! Windows'ta dizin listelemesinin verisini kullandığı için dosya başına ek sistem çağrısı yoktur.

use crate::safety::{plain, Mode, Policy};
use rayon::prelude::*;
use serde::Serialize;
use std::fs::{self, Metadata};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering::Relaxed};
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Temp,
    Browser,
    DevCache,
    DevArtifacts,
    LargeFiles,
    Duplicates,
    /// Geri Dönüşüm Kutusu: listeleme salt-okunur; boşaltma kalıcıdır ve yalnızca kullanıcı seçerse yapılır.
    RecycleBin,
}

impl Category {
    pub fn parse(id: &str) -> Option<Self> {
        match id {
            "temp" => Some(Self::Temp),
            "browser" => Some(Self::Browser),
            "dev_cache" => Some(Self::DevCache),
            "dev_artifacts" => Some(Self::DevArtifacts),
            "large_files" => Some(Self::LargeFiles),
            "duplicates" => Some(Self::Duplicates),
            "recycle_bin" => Some(Self::RecycleBin),
            _ => None,
        }
    }

    /// Kullanıcının kendi dosyalarını listeleyen kategoriler: hiçbir şey önceden seçilmez,
    /// yalnızca Geri Dönüşüm'e taşınır.
    pub fn is_user_files(self) -> bool {
        matches!(self, Self::LargeFiles | Self::Duplicates)
    }
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Item {
    pub path: String,
    pub size_bytes: u64,
    pub file_count: u64,
    /// Son 1 saat içinde değişmiş: büyük olasılıkla kullanımda, varsayılan olarak seçilmemeli.
    pub recently_modified: bool,
    /// İçindeki en yeni değişiklikten bu yana geçen saniye. Bilinmiyorsa 0 (temkinli: yeni sayılır).
    pub age_secs: u64,
    /// Büyük dosya bulucu: temizleme önerisi puanı (0-100). Diğer kategorilerde 0.
    pub score: u8,
    /// Öneri gerekçesi kodu (arayüzde çevrilir): installer, archive, duplicate, ...
    pub reason: Option<&'static str>,
    /// Önerilen mi? (Büyük dosya: puan eşiği üstü; yinelenen: saklanacak kopya dışındakiler.)
    pub recommended: bool,
    /// Yinelenen dosya grubu (aynı içerik). Diğer kategorilerde yok.
    pub group: Option<u32>,
    /// Yinelenen grupta saklanması önerilen kopya.
    pub keep: bool,
}

impl Item {
    pub(crate) fn plain(path: String, size_bytes: u64, file_count: u64, age_secs: u64) -> Self {
        Self {
            path,
            size_bytes,
            file_count,
            recently_modified: age_secs < RECENT_SECS,
            age_secs,
            score: 0,
            reason: None,
            recommended: false,
            group: None,
            keep: false,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ScanResult {
    pub category: Category,
    pub items: Vec<Item>,
    pub total_bytes: u64,
    /// Politika tarafından reddedilen aday sayısı (şeffaflık için).
    pub rejected: u32,
    /// Yaş eşiğinden daha yeni olduğu için listeye alınmayan öğe sayısı.
    pub hidden_recent: u32,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    /// Aday arama: `total` bilinmez (0), `done` şimdiye kadar bulunan sayıdır.
    Discover,
    /// Boyut hesabı: `done / total`.
    Measure,
}

#[derive(Clone, Debug, Serialize)]
pub struct Progress {
    pub category: Category,
    pub phase: Phase,
    pub done: u32,
    pub total: u32,
}

pub type ProgressFn<'a> = &'a (dyn Fn(Progress) + Sync);

pub(crate) const RECENT_SECS: u64 = 3600;
/// Geçici dosyalarda bundan yeni öğeler listelenmez: başka bir uygulamanın çalışma verisi olabilir.
const TEMP_MIN_AGE_SECS: u64 = 7 * 24 * 3600;
const MAX_DEPTH: usize = 6;
/// Geliştirici artığı aranırken hiç girilmeyecek klasörler.
pub(crate) const SKIP_DIRS: &[&str] = &[
    "appdata", "windows", "program files", "program files (x86)", "$recycle.bin",
    "system volume information", ".git", ".cargo", ".rustup",
];

/// Disk G/Ç ağırlıklı olduğundan çekirdek sayısını aşmaz; arayüz için çekirdek bırakır.
fn pool() -> &'static rayon::ThreadPool {
    static POOL: OnceLock<rayon::ThreadPool> = OnceLock::new();
    POOL.get_or_init(|| {
        let n = std::thread::available_parallelism().map_or(4, |n| n.get()).clamp(2, 8);
        rayon::ThreadPoolBuilder::new()
            .num_threads(n)
            .build()
            .expect("rayon thread pool")
    })
}

/// Çocuklarını aday olarak listeleyecek kök klasörler.
pub fn roots_for(cat: Category) -> Vec<PathBuf> {
    let env = |k: &str| std::env::var_os(k).map(PathBuf::from);
    let mut out = Vec::new();
    match cat {
        Category::Temp => {
            out.extend(env("TEMP"));
            out.extend(env("SystemRoot").map(|p| p.join("Temp")));
        }
        Category::Browser => {
            if let Some(local) = env("LOCALAPPDATA") {
                for base in [local.join("Google/Chrome/User Data"), local.join("Microsoft/Edge/User Data")] {
                    for profile in subdirs(&base) {
                        for c in ["Cache", "Code Cache", "GPUCache"] {
                            out.push(profile.join(c));
                        }
                    }
                }
                for profile in subdirs(&local.join("Mozilla/Firefox/Profiles")) {
                    out.push(profile.join("cache2"));
                }
            }
        }
        Category::DevCache => {
            if let Some(local) = env("LOCALAPPDATA") {
                out.push(local.join("npm-cache"));
                out.push(local.join("pip/Cache"));
            }
        }
        Category::DevArtifacts | Category::LargeFiles | Category::Duplicates => out.extend(env("USERPROFILE")),
        Category::RecycleBin => {} // kabuk API'siyle listelenir, dosya sistemi kökü yok
    }
    out.into_iter().filter(|p| p.is_dir()).collect()
}

/// İptal edilirse `None` döner.
pub fn scan(cat: Category, cancel: &AtomicBool, progress: ProgressFn) -> Option<ScanResult> {
    let min_age = if cat == Category::Temp { TEMP_MIN_AGE_SECS } else { 0 };
    scan_roots(cat, &roots_for(cat), None, min_age, cancel, progress)
}

/// Test edilebilirlik için kökler ve (isteğe bağlı) politika dışarıdan verilebilir.
/// `min_age_secs` > 0 ise bundan yeni öğeler listelenmez (yalnızca sayılır).
pub fn scan_roots(
    cat: Category,
    roots: &[PathBuf],
    policy: Option<&Policy>,
    min_age_secs: u64,
    cancel: &AtomicBool,
    progress: ProgressFn,
) -> Option<ScanResult> {
    let default_policy;
    let policy = match policy {
        Some(p) => p,
        None => {
            default_policy = Policy::from_env(roots);
            &default_policy
        }
    };

    if cat.is_user_files() {
        return pool().install(|| crate::finder::scan(cat, roots, policy, cancel, progress));
    }
    if cat == Category::RecycleBin {
        return pool().install(|| crate::recycle::scan(cancel, progress));
    }

    pool().install(|| {
        // 1) Aday arama
        let found = AtomicU32::new(0);
        let candidates: Vec<PathBuf> = match cat {
            Category::LargeFiles | Category::Duplicates | Category::RecycleBin => unreachable!("handled above"),
            Category::DevArtifacts => {
                let out = Mutex::new(Vec::new());
                for root in roots {
                    find_artifacts(root, 0, &out, &found, cat, cancel, progress);
                }
                out.into_inner().unwrap_or_default()
            }
            _ => roots.iter().flat_map(|r| children(r)).collect(),
        };
        if cancel.load(Relaxed) {
            return None;
        }

        // 2) Doğrulama + boyut hesabı (paralel)
        let mode = if cat == Category::DevArtifacts { Mode::DevArtifact } else { Mode::Strict };
        let total = candidates.len() as u32;
        let step = (total / 100).max(1);
        let (done, rejected, hidden) = (AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0));
        progress(Progress { category: cat, phase: Phase::Measure, done: 0, total });

        let mut items: Vec<Item> = candidates
            .par_iter()
            .filter_map(|c| {
                if cancel.load(Relaxed) {
                    return None;
                }
                let item = match policy.validate(c, mode) {
                    Ok(real) => {
                        let item = measure(&real, cancel);
                        if item.age_secs < min_age_secs {
                            hidden.fetch_add(1, Relaxed);
                            None
                        } else {
                            Some(item)
                        }
                    }
                    Err(_) => {
                        rejected.fetch_add(1, Relaxed);
                        None
                    }
                };
                let d = done.fetch_add(1, Relaxed) + 1;
                if d % step == 0 || d == total {
                    progress(Progress { category: cat, phase: Phase::Measure, done: d, total });
                }
                item
            })
            .collect();
        if cancel.load(Relaxed) {
            return None;
        }

        items.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes).then_with(|| a.path.cmp(&b.path)));
        let total_bytes = items.iter().map(|i| i.size_bytes).sum();
        Some(ScanResult {
            category: cat,
            items,
            total_bytes,
            rejected: rejected.load(Relaxed),
            hidden_recent: hidden.load(Relaxed),
        })
    })
}

fn subdirs(p: &Path) -> Vec<PathBuf> {
    children(p).into_iter().filter(|c| c.is_dir()).collect()
}

fn children(p: &Path) -> Vec<PathBuf> {
    fs::read_dir(p).map(|rd| rd.flatten().map(|e| e.path()).collect()).unwrap_or_default()
}

/// Windows: yalnızca bulutta olan (OneDrive vb.) öğe. Bunlara girmek ağdan indirmeyi tetikleyebilir
/// ve yerel diskte yer kaplamazlar.
#[cfg(windows)]
fn cloud_only(m: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const OFFLINE: u32 = 0x1000;
    const RECALL_ON_OPEN: u32 = 0x4_0000;
    const RECALL_ON_DATA_ACCESS: u32 = 0x40_0000;
    m.file_attributes() & (OFFLINE | RECALL_ON_OPEN | RECALL_ON_DATA_ACCESS) != 0
}
#[cfg(not(windows))]
fn cloud_only(_: &Metadata) -> bool {
    false
}

/// Sembolik bağlantı ve junction'lar takip edilmez; bulut yer tutucularına girilmez.
pub(crate) fn skippable(m: &Metadata) -> bool {
    m.file_type().is_symlink() || cloud_only(m)
}

pub fn measure(p: &Path, cancel: &AtomicBool) -> Item {
    let (size_bytes, file_count, newest) = match fs::symlink_metadata(p) {
        Ok(m) if m.is_dir() => {
            let (s, c, n) = dir_stats(p, cancel);
            (s, c, max_time(n, m.modified().ok()))
        }
        Ok(m) if skippable(&m) => (0, 0, None),
        Ok(m) => (m.len(), 1, m.modified().ok()),
        Err(_) => (0, 0, None),
    };
    let age_secs = newest
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .map_or(0, |d| d.as_secs());
    Item::plain(plain(p), size_bytes, file_count, age_secs)
}

fn max_time(a: Option<SystemTime>, b: Option<SystemTime>) -> Option<SystemTime> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (a, b) => a.or(b),
    }
}

type Stats = (u64, u64, Option<SystemTime>);

/// Alt dizinler rayon ile paralel gezilir; erişilemeyenler sessizce atlanır.
fn dir_stats(dir: &Path, cancel: &AtomicBool) -> Stats {
    let Ok(rd) = fs::read_dir(dir) else { return (0, 0, None) };
    let entries: Vec<_> = rd.flatten().collect();
    entries
        .par_iter()
        .map(|e| -> Stats {
            if cancel.load(Relaxed) {
                return (0, 0, None);
            }
            let Ok(m) = e.metadata() else { return (0, 0, None) };
            if skippable(&m) {
                return (0, 0, None);
            }
            if m.is_dir() {
                let (s, c, n) = dir_stats(&e.path(), cancel);
                (s, c, max_time(n, m.modified().ok()))
            } else {
                (m.len(), 1, m.modified().ok())
            }
        })
        .reduce(|| (0, 0, None), |a, b| (a.0 + b.0, a.1 + b.1, max_time(a.2, b.2)))
}

/// Marker dosyasıyla doğrulanan geliştirici artıkları. Bulunan klasörün içine girilmez.
fn find_artifacts(
    dir: &Path,
    depth: usize,
    out: &Mutex<Vec<PathBuf>>,
    found: &AtomicU32,
    cat: Category,
    cancel: &AtomicBool,
    progress: ProgressFn,
) {
    if depth > MAX_DEPTH || cancel.load(Relaxed) {
        return;
    }
    let Ok(rd) = fs::read_dir(dir) else { return };
    let dirs: Vec<(PathBuf, String)> = rd
        .flatten()
        .filter_map(|e| {
            let m = e.metadata().ok()?;
            if !m.is_dir() || skippable(&m) {
                return None;
            }
            let name = e.file_name().to_string_lossy().to_lowercase();
            if SKIP_DIRS.contains(&name.as_str()) {
                return None;
            }
            let path = e.path();
            // Kullanıcının hariç tuttuğu klasörlere hiç girilmez.
            (!crate::settings::is_excluded(&path)).then_some((path, name))
        })
        .collect();

    dirs.par_iter().for_each(|(path, name)| {
        if is_artifact(path, name) {
            if let Ok(mut v) = out.lock() {
                v.push(path.clone());
            }
            let n = found.fetch_add(1, Relaxed) + 1;
            if n % 4 == 0 {
                progress(Progress { category: cat, phase: Phase::Discover, done: n, total: 0 });
            }
        } else {
            find_artifacts(path, depth + 1, out, found, cat, cancel, progress);
        }
    });
}

pub fn is_artifact(path: &Path, name: &str) -> bool {
    let sibling = |f: &str| path.parent().is_some_and(|p| p.join(f).is_file());
    match name {
        "node_modules" | "__pycache__" | ".pytest_cache" => true,
        "target" => sibling("Cargo.toml"),
        "dist" | "build" | ".next" => sibling("package.json"),
        ".gradle" => sibling("build.gradle") || sibling("build.gradle.kts"),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Sandbox(PathBuf);
    impl Sandbox {
        fn new(name: &str) -> Self {
            let p = std::env::temp_dir().join(format!("disktemizle_scan_{name}_{}", std::process::id()));
            let _ = fs::remove_dir_all(&p);
            fs::create_dir_all(&p).unwrap();
            Self(fs::canonicalize(p).unwrap())
        }
        fn file(&self, rel: &str, bytes: usize) -> PathBuf {
            let p = self.0.join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(&p, vec![0u8; bytes]).unwrap();
            p
        }
    }
    impl Drop for Sandbox {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn policy(allowed: &[PathBuf]) -> Policy {
        Policy::new(&[], &[], allowed)
    }

    fn run(cat: Category, roots: &[PathBuf], allowed: &[PathBuf]) -> ScanResult {
        scan_roots(cat, roots, Some(&policy(allowed)), 0, &AtomicBool::new(false), &|_| {}).unwrap()
    }

    #[test]
    fn min_age_hides_recent_items_but_counts_them() {
        let sb = Sandbox::new("age");
        let root = sb.0.join("tmp");
        sb.file("tmp/fresh.bin", 5);
        let r = scan_roots(Category::Temp, &[root.clone()], Some(&policy(&[root])), 7 * 24 * 3600, &AtomicBool::new(false), &|_| {})
            .unwrap();
        assert!(r.items.is_empty());
        assert_eq!(r.hidden_recent, 1);
    }

    #[test]
    fn temp_lists_children_sorted_by_size() {
        let sb = Sandbox::new("temp");
        let root = sb.0.join("tmp");
        sb.file("tmp/small.bin", 10);
        sb.file("tmp/big/a.bin", 500);
        sb.file("tmp/big/b.bin", 500);
        let r = run(Category::Temp, &[root.clone()], &[root]);
        assert_eq!(r.items.len(), 2);
        assert_eq!(r.items[0].size_bytes, 1000);
        assert_eq!(r.items[0].file_count, 2);
        assert_eq!(r.total_bytes, 1010);
        assert!(r.items[0].recently_modified);
    }

    #[test]
    fn nested_dirs_are_summed() {
        let sb = Sandbox::new("nested");
        let root = sb.0.join("tmp");
        for i in 0..20 {
            sb.file(&format!("tmp/d/a{i}/b/c/f.bin"), 100);
        }
        let r = run(Category::Temp, &[root.clone()], &[root]);
        assert_eq!(r.items[0].size_bytes, 2000);
        assert_eq!(r.items[0].file_count, 20);
    }

    #[test]
    fn scan_never_modifies_files() {
        let sb = Sandbox::new("ro");
        let root = sb.0.join("tmp");
        let f = sb.file("tmp/x.bin", 5);
        run(Category::Temp, &[root.clone()], &[root]);
        assert!(f.exists());
    }

    #[test]
    fn policy_rejections_are_counted_not_listed() {
        let sb = Sandbox::new("rej");
        let root = sb.0.join("tmp");
        sb.file("tmp/ok.bin", 1);
        sb.file("tmp/.git/config", 1);
        let r = run(Category::Temp, &[root.clone()], &[root]);
        assert_eq!(r.items.len(), 1);
        assert_eq!(r.rejected, 1);
    }

    #[test]
    fn dev_artifacts_need_markers_and_are_not_descended() {
        let sb = Sandbox::new("dev");
        sb.file("p1/package.json", 1);
        sb.file("p1/node_modules/dep/index.js", 100);
        sb.file("p1/node_modules/dep/node_modules/inner/i.js", 100);
        sb.file("p2/Cargo.toml", 1);
        sb.file("p2/target/debug/x", 50);
        sb.file("p3/target/keep.txt", 7); // Cargo.toml yok: artık sayılmaz
        let r = run(Category::DevArtifacts, &[sb.0.clone()], &[sb.0.clone()]);
        let paths: Vec<_> = r.items.iter().map(|i| i.path.clone()).collect();
        assert_eq!(paths.len(), 2, "{paths:?}");
        assert!(paths.iter().any(|p| p.ends_with("node_modules")));
        assert!(paths.iter().any(|p| p.ends_with("target") && p.contains("p2")));
        assert!(!paths.iter().any(|p| p.contains("p3")));
    }

    #[test]
    fn cancel_returns_none() {
        let sb = Sandbox::new("cancel");
        let root = sb.0.join("tmp");
        sb.file("tmp/a.bin", 1);
        let cancel = AtomicBool::new(true);
        assert!(scan_roots(Category::Temp, &[root.clone()], Some(&policy(&[root])), 0, &cancel, &|_| {}).is_none());
    }

    #[test]
    fn progress_reports_measure_phase_completion() {
        let sb = Sandbox::new("prog");
        let root = sb.0.join("tmp");
        for i in 0..5 {
            sb.file(&format!("tmp/f{i}.bin"), 1);
        }
        let last = Mutex::new(None);
        scan_roots(Category::Temp, &[root.clone()], Some(&policy(&[root])), 0, &AtomicBool::new(false), &|p| {
            *last.lock().unwrap() = Some(p);
        });
        let p = last.into_inner().unwrap().unwrap();
        assert_eq!((p.phase, p.done, p.total), (Phase::Measure, 5, 5));
    }

    /// Gerçek disk üzerinde salt-okunur tarama özeti. `cargo test --release -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn real_scan_report() {
        for cat in [Category::Temp, Category::Browser, Category::DevCache, Category::DevArtifacts] {
            let t = std::time::Instant::now();
            let r = scan(cat, &AtomicBool::new(false), &|_| {}).unwrap();
            println!(
                "{cat:?}: {} items, {:.1} MB, rejected {}, {:.1}s",
                r.items.len(),
                r.total_bytes as f64 / 1048576.0,
                r.rejected,
                t.elapsed().as_secs_f32()
            );
            for i in r.items.iter().take(5) {
                println!("   {:>9.1} MB  {}{}", i.size_bytes as f64 / 1048576.0, i.path, if i.recently_modified { "  [recent]" } else { "" });
            }
        }
    }

    #[test]
    fn category_parse_roundtrip() {
        assert_eq!(Category::parse("temp"), Some(Category::Temp));
        assert_eq!(Category::parse("dev_artifacts"), Some(Category::DevArtifacts));
        assert_eq!(Category::parse("nope"), None);
    }
}
