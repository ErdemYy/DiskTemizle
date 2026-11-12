//! Büyük ve yinelenen dosya bulucu. Tarama salt-okunurdur; yalnızca öneri üretir.
//! Silme `cleanup` üzerinden, yalnızca Geri Dönüşüm'e taşıyarak yapılır.

use crate::safety::{plain, Mode, Policy};
use crate::scan::{is_artifact, skippable, Category, Item, Phase, Progress, ProgressFn, ScanResult, SKIP_DIRS};
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering::Relaxed};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::SystemTime;

const MB: u64 = 1024 * 1024;
pub const LARGE_MIN: u64 = 100 * MB;
pub const DUP_MIN: u64 = MB;
const LARGE_LIMIT: usize = 300;
const WALK_DEPTH: usize = 16;
const PARTIAL_BYTES: u64 = 64 * 1024;
/// Bu puanın üstündeki büyük dosyalar "önerilen" sayılır.
pub const RECOMMEND_MIN: u8 = 60;
/// Bağımlılık ve ortam klasörlerine girilmez. Derleme çıktıları (`target`, `dist`...) ayrıca
/// `is_artifact` ile elenir: onlar "proje artıkları" kategorisinin işi ve kopyaları gürültüdür.
const EXTRA_SKIP: &[&str] = &["node_modules", "__pycache__", "venv", ".venv", "site-packages"];

struct Ctx<'a> {
    cat: Category,
    cancel: &'a AtomicBool,
    progress: ProgressFn<'a>,
}

pub struct FileInfo {
    pub path: PathBuf,
    pub size: u64,
    pub modified: Option<SystemTime>,
}

fn age_secs(modified: Option<SystemTime>) -> u64 {
    modified
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .map_or(0, |d| d.as_secs())
}

/// Paralel dosya yürüyüşü; `min_size` altındaki, bulut-yalnız ve sembolik bağlantı dosyalar atlanır.
fn walk(dir: &Path, depth: usize, min_size: u64, out: &Mutex<Vec<FileInfo>>, found: &AtomicU32, ctx: &Ctx) {
    if depth > WALK_DEPTH || ctx.cancel.load(Relaxed) {
        return;
    }
    let Ok(rd) = fs::read_dir(dir) else { return };
    let (mut files, mut dirs) = (Vec::new(), Vec::new());
    for e in rd.flatten() {
        let Ok(m) = e.metadata() else { continue };
        if skippable(&m) {
            continue;
        }
        if m.is_dir() {
            let name = e.file_name().to_string_lossy().to_lowercase();
            if SKIP_DIRS.contains(&name.as_str()) || EXTRA_SKIP.contains(&name.as_str()) {
                continue;
            }
            let path = e.path();
            if !is_artifact(&path, &name) {
                dirs.push(path);
            }
        } else if m.is_file() && m.len() >= min_size {
            files.push(FileInfo { path: e.path(), size: m.len(), modified: m.modified().ok() });
        }
    }
    if !files.is_empty() {
        let n = files.len() as u32;
        if let Ok(mut v) = out.lock() {
            v.extend(files);
        }
        let now = found.fetch_add(n, Relaxed) + n;
        if now / 16 != (now - n) / 16 {
            (ctx.progress)(Progress { category: ctx.cat, phase: Phase::Discover, done: now, total: 0 });
        }
    }
    dirs.par_iter().for_each(|d| walk(d, depth + 1, min_size, out, found, ctx));
}

fn collect(roots: &[PathBuf], min_size: u64, ctx: &Ctx) -> Option<Vec<FileInfo>> {
    let (out, found) = (Mutex::new(Vec::new()), AtomicU32::new(0));
    for r in roots {
        walk(r, 0, min_size, &out, &found, ctx);
    }
    if ctx.cancel.load(Relaxed) {
        return None;
    }
    Some(out.into_inner().unwrap_or_default())
}

pub fn scan(cat: Category, roots: &[PathBuf], policy: &Policy, cancel: &AtomicBool, progress: ProgressFn) -> Option<ScanResult> {
    let ctx = Ctx { cat, cancel, progress };
    match cat {
        Category::LargeFiles => scan_large(roots, policy, &ctx, LARGE_MIN),
        Category::Duplicates => scan_dups(roots, policy, &ctx, DUP_MIN),
        _ => unreachable!("not a finder category"),
    }
}

// ---------------------------------------------------------------- Büyük dosyalar

/// Temizleme önerisi puanı (0-100) ve gerekçe kodu. Saf fonksiyon: dosya sistemine bakmaz.
/// Kişisel belge ve medya düşük puan alır; kurulum dosyaları, döküm/günlük ve eski arşivler yüksek.
pub fn recommend(path: &Path, size: u64, age_secs: u64) -> (u8, &'static str) {
    let lower = path.to_string_lossy().to_lowercase();
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let in_downloads = lower.contains("\\downloads\\");

    let (base, reason): (i32, &'static str) = match ext.as_str() {
        "dmp" | "mdmp" | "etl" | "log" | "tmp" | "bak" | "old" | "crdownload" | "part" => (70, "dump_or_log"),
        "msi" | "msix" | "appx" | "exe" if in_downloads => (60, "installer"),
        "iso" | "img" | "dmg" => (55, "disk_image"),
        "zip" | "7z" | "rar" | "tar" | "gz" | "tgz" | "bz2" | "xz" => (50, "archive"),
        "mp4" | "mkv" | "avi" | "mov" | "wmv" | "mp3" | "flac" | "wav" => (15, "media"),
        "vmdk" | "vdi" | "vhd" | "vhdx" | "qcow2" | "db" | "sqlite" | "pst" | "ost" | "kdbx" => (5, "data_file"),
        "psd" | "ai" | "blend" | "docx" | "xlsx" | "pptx" | "pdf" | "dwg" => (5, "personal_doc"),
        _ => (25, "other"),
    };

    const DAY: u64 = 24 * 3600;
    let age = match age_secs {
        a if a > 365 * DAY => 25,
        a if a > 180 * DAY => 15,
        a if a > 90 * DAY => 8,
        a if a < 30 * DAY => -15,
        _ => 0,
    };
    let loc = if in_downloads { 10 } else { 0 };
    let sz = match size {
        s if s >= 5 * 1024 * MB => 15,
        s if s >= 1024 * MB => 10,
        _ => 0,
    };
    ((base + age + loc + sz).clamp(0, 100) as u8, reason)
}

fn scan_large(roots: &[PathBuf], policy: &Policy, ctx: &Ctx, min_size: u64) -> Option<ScanResult> {
    let mut files = collect(roots, min_size, ctx)?;
    files.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.path.cmp(&b.path)));
    files.truncate(LARGE_LIMIT);

    let (mut items, mut rejected) = (Vec::new(), 0u32);
    for f in files {
        match policy.validate(&f.path, Mode::UserFile) {
            Ok(real) => {
                let age = age_secs(f.modified);
                let (score, reason) = recommend(&real, f.size, age);
                let mut item = Item::plain(plain(&real), f.size, 1, age);
                item.score = score;
                item.reason = Some(reason);
                item.recommended = score >= RECOMMEND_MIN;
                items.push(item);
            }
            Err(_) => rejected += 1,
        }
    }
    // "Kazanılabilir" = yalnızca önerilenler; kullanıcı gerisini kendi kararıyla seçer.
    let total_bytes = items.iter().filter(|i| i.recommended).map(|i| i.size_bytes).sum();
    Some(ScanResult { category: ctx.cat, items, total_bytes, rejected, hidden_recent: 0 })
}

// ---------------------------------------------------------------- Yinelenen dosyalar

type Hash = [u8; 32];

/// Saklanacak kopya seçiminde konum önceliği (küçük = saklamaya daha uygun): düzenli klasörler,
/// sonra Masaüstü, en sonda İndirilenler. Böylece silme önerisi önce İndirilenler'deki kopyalara düşer.
fn keep_rank(p: &Path) -> u8 {
    let l = p.to_string_lossy().to_lowercase();
    if l.contains("\\downloads\\") || l.contains("\\indirilenler\\") {
        2
    } else if l.contains("\\desktop\\") || l.contains("\\masaüstü\\") {
        1
    } else {
        0
    }
}

fn hash_partial(p: &Path) -> Option<Hash> {
    let mut buf = Vec::with_capacity(PARTIAL_BYTES as usize);
    File::open(p).ok()?.take(PARTIAL_BYTES).read_to_end(&mut buf).ok()?;
    Some(*blake3::hash(&buf).as_bytes())
}

fn hash_full(p: &Path) -> Option<Hash> {
    let mut h = blake3::Hasher::new();
    h.update_reader(File::open(p).ok()?).ok()?;
    Some(*h.finalize().as_bytes())
}

/// Silme anında kullanılan kayıt: tarama sonrası dosyanın değişmediğini ve grupta
/// başka bir kopya kaldığını doğrulamak için.
#[derive(Default)]
struct DupIndex {
    /// anahtar (küçük harf yol) -> (grup, boyut, değişiklik zamanı)
    entries: HashMap<String, (u32, u64, Option<SystemTime>)>,
    /// grup -> (anahtar, özgün yol)
    groups: HashMap<u32, Vec<(String, String)>>,
}

fn index() -> MutexGuard<'static, DupIndex> {
    static INDEX: OnceLock<Mutex<DupIndex>> = OnceLock::new();
    INDEX.get_or_init(Mutex::default).lock().unwrap_or_else(|e| e.into_inner())
}

static NEXT_GROUP: AtomicU32 = AtomicU32::new(1);

fn key(path: &str) -> String {
    path.to_lowercase()
}

/// Silinecek yollar için atlama gerekçesi (`None` = silinebilir). Üç kural:
/// taramadan gelmeli, taramadan beri değişmemeli, her grupta en az bir kopya kalmalı.
/// Eski taramaların kayıtları da geçerlidir: tüm denetimler dosyanın o anki durumuna bakar.
pub fn dup_guard(paths: &[String]) -> Vec<Option<&'static str>> {
    let idx = index();
    let selected: HashSet<String> = paths.iter().map(|p| key(p)).collect();
    paths
        .iter()
        .map(|p| {
            let Some((group, size, mtime)) = idx.entries.get(&key(p)) else { return Some("no_scan_record") };
            let Ok(m) = fs::metadata(p) else { return Some("missing") };
            if m.len() != *size || m.modified().ok() != *mtime {
                return Some("changed_since_scan");
            }
            let remaining = idx.groups.get(group).map_or(0, |members| {
                members.iter().filter(|(k, orig)| !selected.contains(k) && fs::metadata(orig).is_ok()).count()
            });
            (remaining == 0).then_some("last_copy")
        })
        .collect()
}

fn scan_dups(roots: &[PathBuf], policy: &Policy, ctx: &Ctx, min_size: u64) -> Option<ScanResult> {
    let files = collect(roots, min_size, ctx)?;

    // 1) Aynı boyutlular
    let mut by_size: HashMap<u64, Vec<usize>> = HashMap::new();
    for (i, f) in files.iter().enumerate() {
        by_size.entry(f.size).or_default().push(i);
    }
    let candidates: Vec<usize> = by_size.values().filter(|v| v.len() >= 2).flatten().copied().collect();

    let done = AtomicU32::new(0);
    let tick = |total: u32| {
        let d = done.fetch_add(1, Relaxed) + 1;
        if d % 16 == 0 || d == total {
            (ctx.progress)(Progress { category: ctx.cat, phase: Phase::Measure, done: d, total });
        }
    };
    let total1 = candidates.len() as u32;
    (ctx.progress)(Progress { category: ctx.cat, phase: Phase::Measure, done: 0, total: total1 });

    // 2) İlk 64 KB hash'i: çoğu aday burada elenir, tüm dosya okunmaz.
    let partial: Vec<(usize, Option<Hash>)> = candidates
        .par_iter()
        .map(|&i| {
            let h = if ctx.cancel.load(Relaxed) { None } else { hash_partial(&files[i].path) };
            tick(total1);
            (i, h)
        })
        .collect();
    if ctx.cancel.load(Relaxed) {
        return None;
    }
    let mut by_partial: HashMap<(u64, Hash), Vec<usize>> = HashMap::new();
    for (i, h) in partial {
        if let Some(h) = h {
            by_partial.entry((files[i].size, h)).or_default().push(i);
        }
    }
    let survivors: Vec<usize> = by_partial.values().filter(|v| v.len() >= 2).flatten().copied().collect();

    // 3) Tam hash yalnızca kalanlar için
    let total2 = total1 + survivors.len() as u32;
    let full: Vec<(usize, Option<Hash>)> = survivors
        .par_iter()
        .map(|&i| {
            let h = if ctx.cancel.load(Relaxed) { None } else { hash_full(&files[i].path) };
            tick(total2);
            (i, h)
        })
        .collect();
    if ctx.cancel.load(Relaxed) {
        return None;
    }
    let mut by_full: HashMap<(u64, Hash), Vec<usize>> = HashMap::new();
    for (i, h) in full {
        if let Some(h) = h {
            by_full.entry((files[i].size, h)).or_default().push(i);
        }
    }

    // 4) Gruplar: güvenlik kapısından geçen üyeler; saklanacak kopya = en eski, eşitse en kısa yol.
    struct Group {
        id: u32,
        members: Vec<(Item, Option<SystemTime>)>,
    }
    let (mut groups, mut rejected) = (Vec::<Group>::new(), 0u32);
    for idxs in by_full.into_values().filter(|v| v.len() >= 2) {
        let mut valid: Vec<(PathBuf, usize)> = Vec::new();
        for i in idxs {
            match policy.validate(&files[i].path, Mode::UserFile) {
                Ok(real) => valid.push((real, i)),
                Err(_) => rejected += 1,
            }
        }
        // Korumalı konumdaki bir kopya listelenmez; geriye 2'den az kalırsa grup öneri üretmez.
        if valid.len() < 2 {
            continue;
        }
        valid.sort_by(|(pa, ia), (pb, ib)| {
            let (ma, mb) = (files[*ia].modified, files[*ib].modified);
            keep_rank(pa)
                .cmp(&keep_rank(pb))
                .then_with(|| ma.cmp(&mb))
                .then_with(|| pa.as_os_str().len().cmp(&pb.as_os_str().len()))
                .then_with(|| pa.cmp(pb))
        });
        let id = NEXT_GROUP.fetch_add(1, Relaxed);
        let members = valid
            .into_iter()
            .enumerate()
            .map(|(n, (real, i))| {
                let f = &files[i];
                let mut item = Item::plain(plain(&real), f.size, 1, age_secs(f.modified));
                item.group = Some(id);
                item.keep = n == 0;
                item.recommended = n != 0;
                item.reason = Some("duplicate");
                (item, f.modified)
            })
            .collect();
        groups.push(Group { id, members });
    }
    groups.sort_by_key(|g| {
        let s = g.members[0].0.size_bytes;
        std::cmp::Reverse(s * (g.members.len() as u64 - 1))
    });

    // Silme anı denetimi için kayıt (var olanla birleşir; bkz. `dup_guard`).
    {
        let mut idx = index();
        for g in &groups {
            let mut list = Vec::new();
            for (item, mtime) in &g.members {
                let k = key(&item.path);
                idx.entries.insert(k.clone(), (g.id, item.size_bytes, *mtime));
                list.push((k, item.path.clone()));
            }
            idx.groups.insert(g.id, list);
        }
    }

    let total_bytes = groups.iter().map(|g| g.members[0].0.size_bytes * (g.members.len() as u64 - 1)).sum();
    let items = groups.into_iter().flat_map(|g| g.members.into_iter().map(|(i, _)| i)).collect();
    Some(ScanResult { category: ctx.cat, items, total_bytes, rejected, hidden_recent: 0 })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    struct Sandbox(PathBuf);
    impl Sandbox {
        fn new(name: &str) -> Self {
            let p = std::env::temp_dir().join(format!("disktemizle_find_{name}_{}", std::process::id()));
            let _ = fs::remove_dir_all(&p);
            fs::create_dir_all(&p).unwrap();
            Self(fs::canonicalize(p).unwrap())
        }
        fn file(&self, rel: &str, content: &[u8]) -> PathBuf {
            let p = self.0.join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(&p, content).unwrap();
            p
        }
    }
    impl Drop for Sandbox {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn ctx<'a>(cat: Category, cancel: &'a AtomicBool) -> Ctx<'a> {
        Ctx { cat, cancel, progress: &|_| {} }
    }
    fn policy(sb: &Sandbox) -> Policy {
        Policy::new(&[], &[], &[sb.0.clone()])
    }
    const DAY: u64 = 24 * 3600;

    #[test]
    fn recommend_prefers_installers_and_logs_over_personal_files() {
        let old = 400 * DAY;
        let (s, r) = recommend(Path::new("C:\\Users\\a\\Downloads\\setup.exe"), 300 * MB, old);
        assert!(s >= RECOMMEND_MIN && r == "installer", "{s} {r}");
        let (s, r) = recommend(Path::new("C:\\x\\crash.dmp"), 300 * MB, old);
        assert!(s >= RECOMMEND_MIN && r == "dump_or_log");
        let (s, r) = recommend(Path::new("C:\\Users\\a\\Documents\\thesis.docx"), 300 * MB, old);
        assert!(s < RECOMMEND_MIN && r == "personal_doc");
        let (s, _) = recommend(Path::new("C:\\Users\\a\\Videos\\holiday.mp4"), 2000 * MB, old);
        assert!(s < RECOMMEND_MIN);
    }

    #[test]
    fn recommend_penalises_recent_files() {
        let p = Path::new("C:\\Users\\a\\Downloads\\old.zip");
        let (old, _) = recommend(p, 300 * MB, 400 * DAY);
        let (fresh, _) = recommend(p, 300 * MB, DAY);
        assert!(fresh < old);
    }

    #[test]
    fn large_files_are_sorted_and_only_recommended_count_as_reclaimable() {
        let sb = Sandbox::new("large");
        sb.file("a/big.log", &vec![1u8; 500]);
        sb.file("a/mid.docx", &vec![2u8; 300]);
        sb.file("a/tiny.bin", &vec![3u8; 5]);
        let cancel = AtomicBool::new(false);
        let r = scan_large(&[sb.0.clone()], &policy(&sb), &ctx(Category::LargeFiles, &cancel), 100).unwrap();
        assert_eq!(r.items.len(), 2);
        assert!(r.items[0].path.ends_with("big.log") && r.items[1].path.ends_with("mid.docx"));
        // Yeni dosyalar yaş cezası alır; yine de log/dökümün puanı belgeden yüksektir.
        assert!(r.items[0].score > r.items[1].score);
        assert!(r.items.iter().all(|i| i.group.is_none() && !i.keep));
    }

    #[test]
    fn duplicates_group_identical_content_only() {
        let sb = Sandbox::new("dups");
        let same = vec![7u8; 4000];
        let a = sb.file("a/one.bin", &same);
        std::thread::sleep(Duration::from_millis(30));
        sb.file("b/two.bin", &same);
        sb.file("b/sub/three.bin", &same);
        // Aynı boyut, farklı içerik: yinelenen sayılmamalı.
        let mut other = same.clone();
        other[3999] = 8;
        sb.file("c/other.bin", &other);
        sb.file("c/unique.bin", &vec![9u8; 1234]);

        let cancel = AtomicBool::new(false);
        let r = scan_dups(&[sb.0.clone()], &policy(&sb), &ctx(Category::Duplicates, &cancel), 100).unwrap();
        assert_eq!(r.items.len(), 3, "{:?}", r.items.iter().map(|i| &i.path).collect::<Vec<_>>());
        assert_eq!(r.items.iter().filter(|i| i.keep).count(), 1);
        assert_eq!(r.items.iter().filter(|i| i.recommended).count(), 2);
        assert!(r.items.iter().all(|i| i.group == r.items[0].group));
        assert_eq!(r.total_bytes, 4000 * 2);
        // En eski dosya saklanır.
        let keeper = r.items.iter().find(|i| i.keep).unwrap();
        assert!(keeper.path.ends_with("one.bin"), "{}", keeper.path);
        assert!(a.exists());
    }

    #[test]
    fn keeper_prefers_organised_folders_over_downloads() {
        let sb = Sandbox::new("keeper");
        let same = vec![4u8; 3000];
        sb.file("Downloads/old.bin", &same); // önce yazıldı: en eskisi
        std::thread::sleep(Duration::from_millis(30));
        sb.file("projects/kept.bin", &same);
        let cancel = AtomicBool::new(false);
        let r = scan_dups(&[sb.0.clone()], &policy(&sb), &ctx(Category::Duplicates, &cancel), 100).unwrap();
        let keeper = r.items.iter().find(|i| i.keep).unwrap();
        assert!(keeper.path.ends_with("kept.bin"), "{}", keeper.path);
        assert!(r.items.iter().find(|i| i.recommended).unwrap().path.contains("Downloads"));
    }

    #[test]
    fn build_artifacts_are_not_scanned() {
        let sb = Sandbox::new("artifacts");
        let same = vec![3u8; 3000];
        sb.file("proj/Cargo.toml", b"[package]");
        sb.file("proj/target/debug/a.bin", &same);
        sb.file("proj/target/debug/deps/a.bin", &same);
        sb.file("web/package.json", b"{}");
        sb.file("web/node_modules/x/big.bin", &same);
        let cancel = AtomicBool::new(false);
        let d = scan_dups(&[sb.0.clone()], &policy(&sb), &ctx(Category::Duplicates, &cancel), 100).unwrap();
        assert!(d.items.is_empty(), "{:?}", d.items.iter().map(|i| &i.path).collect::<Vec<_>>());
        let l = scan_large(&[sb.0.clone()], &policy(&sb), &ctx(Category::LargeFiles, &cancel), 100).unwrap();
        assert!(l.items.is_empty());
    }

    #[test]
    fn guard_never_allows_removing_every_copy() {
        let sb = Sandbox::new("guard");
        let same = vec![5u8; 2000];
        for n in ["a/1.bin", "b/2.bin", "c/3.bin"] {
            sb.file(n, &same);
        }
        let cancel = AtomicBool::new(false);
        let r = scan_dups(&[sb.0.clone()], &policy(&sb), &ctx(Category::Duplicates, &cancel), 100).unwrap();
        let paths: Vec<String> = r.items.iter().map(|i| i.path.clone()).collect();

        // İkisini seçmek serbest, üçünü birden seçmek değil.
        assert!(dup_guard(&paths[1..]).iter().all(Option::is_none));
        assert!(dup_guard(&paths).iter().all(|g| *g == Some("last_copy")));
    }

    #[test]
    fn guard_rejects_unknown_and_changed_files() {
        let sb = Sandbox::new("guard2");
        let same = vec![5u8; 2000];
        let a = sb.file("a/1.bin", &same);
        sb.file("b/2.bin", &same);
        let cancel = AtomicBool::new(false);
        let r = scan_dups(&[sb.0.clone()], &policy(&sb), &ctx(Category::Duplicates, &cancel), 100).unwrap();
        let victim = r.items.iter().find(|i| i.recommended).unwrap().path.clone();

        assert_eq!(dup_guard(&["C:\\never\\scanned.bin".into()]), vec![Some("no_scan_record")]);

        // Taramadan sonra değiştirilen dosya silinmez.
        fs::write(&victim, vec![6u8; 2500]).unwrap();
        assert_eq!(dup_guard(&[victim]), vec![Some("changed_since_scan")]);
        assert!(a.exists());
    }

    /// Gerçek profil üzerinde salt-okunur özet. `cargo test --release -- --ignored --nocapture real_finder`.
    #[test]
    #[ignore]
    fn real_finder_report() {
        for cat in [Category::LargeFiles, Category::Duplicates] {
            let t = std::time::Instant::now();
            let roots = crate::scan::roots_for(cat);
            let policy = Policy::from_env(&roots);
            let r = scan(cat, &roots, &policy, &AtomicBool::new(false), &|_| {}).unwrap();
            println!(
                "{cat:?}: {} items, reclaimable {:.1} MB, rejected {}, {:.1}s",
                r.items.len(),
                r.total_bytes as f64 / 1048576.0,
                r.rejected,
                t.elapsed().as_secs_f32()
            );
            for i in r.items.iter().take(8) {
                println!(
                    "   {:>8.1} MB  score {:>3} {:<12} keep={} grp={:?}  {}",
                    i.size_bytes as f64 / 1048576.0,
                    i.score,
                    i.reason.unwrap_or("-"),
                    i.keep,
                    i.group,
                    i.path
                );
            }
        }
    }

    #[test]
    fn cancel_returns_none() {
        let sb = Sandbox::new("cancel");
        sb.file("a/x.bin", &[1u8; 500]);
        let cancel = AtomicBool::new(true);
        assert!(scan_large(&[sb.0.clone()], &policy(&sb), &ctx(Category::LargeFiles, &cancel), 100).is_none());
        assert!(scan_dups(&[sb.0.clone()], &policy(&sb), &ctx(Category::Duplicates, &cancel), 100).is_none());
    }
}
