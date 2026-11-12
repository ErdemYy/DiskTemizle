//! Silme servisi. Arayüzden gelen yollara güvenmez: her yol burada yeniden
//! `safety::Policy` ile doğrulanır. Varsayılan eylem Geri Dönüşüm'e taşımadır.

use crate::finder;
use crate::safety::{plain, Mode, Policy};
use crate::scan::{self, Category};
use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::{SystemTime, UNIX_EPOCH};

pub trait Remover {
    fn trash(&self, path: &Path) -> Result<(), String>;
    fn remove_permanent(&self, path: &Path) -> Result<(), String>;
    fn restore(&self, original: &Path) -> Result<(), String>;
}

pub struct SystemRemover;

impl Remover for SystemRemover {
    fn trash(&self, path: &Path) -> Result<(), String> {
        trash::delete(path).map_err(|e| e.to_string())
    }

    fn remove_permanent(&self, path: &Path) -> Result<(), String> {
        let meta = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
        if meta.is_dir() { fs::remove_dir_all(path) } else { fs::remove_file(path) }.map_err(|e| e.to_string())
    }

    fn restore(&self, original: &Path) -> Result<(), String> {
        let want = plain(original).to_lowercase();
        let mut found: Vec<_> = trash::os_limited::list()
            .map_err(|e| e.to_string())?
            .into_iter()
            .filter(|i| plain(&i.original_path()).to_lowercase() == want)
            .collect();
        // Aynı yoldan birden fazla varsa en son silineni geri yükle.
        found.sort_by_key(|i| i.time_deleted);
        let latest = found.pop().ok_or("not found in Recycle Bin")?;
        trash::os_limited::restore_all([latest]).map_err(|e| e.to_string())
    }
}

#[derive(Debug, Serialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Trashed,
    Deleted,
    Skipped,
    Failed,
    Restored,
}

#[derive(Debug, Serialize)]
pub struct Outcome {
    pub path: String,
    pub action: Action,
    pub bytes: u64,
    pub detail: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub outcomes: Vec<Outcome>,
    pub freed_bytes: u64,
}

/// Günlük: her satır bir JSON kaydı. `open` yazılabilirliği baştan doğrular; bir kayıt
/// yazılamazsa sonraki öğelere geçilmez.
pub struct Journal(PathBuf);

impl Journal {
    pub fn open(path: PathBuf) -> Result<Self, String> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        OpenOptions::new().create(true).append(true).open(&path).map_err(|e| e.to_string())?;
        Ok(Self(path))
    }

    fn record(&self, cat: &str, o: &Outcome) -> Result<(), String> {
        let ts = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let line = serde_json::json!({ "ts": ts, "category": cat, "path": o.path,
            "action": o.action, "bytes": o.bytes, "detail": o.detail });
        let mut f = OpenOptions::new().append(true).open(&self.0).map_err(|e| e.to_string())?;
        writeln!(f, "{line}").map_err(|e| e.to_string())
    }
}

/// Günlükten okunan, arayüze gösterilen kayıt.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Entry {
    pub ts: u64,
    /// Kategori kimliği (`dev_cache`...) ya da geri alma kayıtları için `undo`.
    pub category: String,
    pub path: String,
    pub action: String,
    pub bytes: u64,
    pub detail: Option<String>,
    /// Geri Dönüşüm'e taşınmış ve sonradan geri yüklenmemiş son kayıt.
    pub undoable: bool,
}

/// Günlüğün yalnızca kuyruğu okunur; dosya yıllar içinde büyüse de ekran hızlı açılır.
const TAIL_BYTES: u64 = 4 * 1024 * 1024;

fn snake_case(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            out.push('_');
        }
        out.extend(c.to_lowercase());
    }
    out
}

impl Journal {
    /// En yeni kayıt başta, en fazla `limit` kayıt. Bozuk satırlar atlanır.
    pub fn read(&self, limit: usize) -> Result<Vec<Entry>, String> {
        use std::io::{Read, Seek, SeekFrom};
        let mut f = fs::File::open(&self.0).map_err(|e| e.to_string())?;
        let len = f.metadata().map_err(|e| e.to_string())?.len();
        let start = len.saturating_sub(TAIL_BYTES);
        f.seek(SeekFrom::Start(start)).map_err(|e| e.to_string())?;
        let mut bytes = Vec::new();
        f.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        let text = String::from_utf8_lossy(&bytes);
        // Kuyruk bir satırın ortasından başlayabilir: ilk (eksik) satırı at.
        let skip = usize::from(start > 0);

        let mut entries: Vec<Entry> = text
            .lines()
            .skip(skip)
            .filter_map(|line| {
                let v: serde_json::Value = serde_json::from_str(line).ok()?;
                Some(Entry {
                    ts: v.get("ts")?.as_u64()?,
                    category: snake_case(v.get("category")?.as_str()?),
                    path: v.get("path")?.as_str()?.to_string(),
                    action: v.get("action")?.as_str()?.to_string(),
                    bytes: v.get("bytes").and_then(|b| b.as_u64()).unwrap_or(0),
                    detail: v.get("detail").and_then(|d| d.as_str()).map(str::to_string),
                    undoable: false,
                })
            })
            .collect();

        // Kronolojik tara: aynı yol için yalnızca son "trashed" kaydı ve ardından geri yüklenmemişse geri alınabilir.
        let mut last_trashed: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for (i, e) in entries.iter().enumerate() {
            let k = e.path.to_lowercase();
            match e.action.as_str() {
                "trashed" => {
                    last_trashed.insert(k, i);
                }
                "restored" => {
                    last_trashed.remove(&k);
                }
                _ => {}
            }
        }
        for i in last_trashed.into_values() {
            entries[i].undoable = true;
        }

        entries.reverse();
        entries.truncate(limit);
        Ok(entries)
    }
}

fn mode_for(cat: Category) -> Mode {
    match cat {
        Category::DevArtifacts => Mode::DevArtifact,
        Category::LargeFiles | Category::Duplicates => Mode::UserFile,
        _ => Mode::Strict,
    }
}

/// Çekirdek mantık; politika/kaldırıcı/günlük dışarıdan verildiği için test edilebilir.
pub fn run_with(
    cat: Category,
    paths: &[String],
    permanent: bool,
    policy: &Policy,
    remover: &dyn Remover,
    journal: &Journal,
) -> Result<Report, String> {
    // Kalıcı silme yalnızca yeniden üretilebilir önbellek kategorilerinde; geliştirici artığında
    // ve kullanıcının kendi dosyalarında asla.
    let permanent = permanent && cat != Category::DevArtifacts && !cat.is_user_files();
    let cat_name = format!("{cat:?}");
    let mut report = Report { outcomes: Vec::new(), freed_bytes: 0 };
    // Yinelenenlerde: taramadan gelmeli, değişmemeli ve her grupta en az bir kopya kalmalı.
    let guard = if cat == Category::Duplicates { finder::dup_guard(paths) } else { vec![None; paths.len()] };

    for (n, raw) in paths.iter().enumerate() {
        let skip = |why: String| Outcome { path: raw.clone(), action: Action::Skipped, bytes: 0, detail: Some(why) };
        let outcome = match policy.validate(Path::new(raw), mode_for(cat)) {
            Err(e) => skip(e.to_string()),
            Ok(real) => {
                let name = real.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
                if cat == Category::DevArtifacts && !scan::is_artifact(&real, &name) {
                    skip("not a recognised developer artifact".into())
                } else if cat.is_user_files() && !real.is_file() {
                    skip("not a file".into())
                } else if let Some(why) = guard[n] {
                    skip(why.into())
                } else {
                    let bytes = scan::measure(&real, &AtomicBool::new(false)).size_bytes;
                    let res = if permanent { remover.remove_permanent(&real) } else { remover.trash(&real) };
                    match res {
                        Ok(()) => Outcome {
                            path: plain(&real),
                            action: if permanent { Action::Deleted } else { Action::Trashed },
                            bytes,
                            detail: None,
                        },
                        // Kilitli/kullanımdaki dosya hata değil, atlanan öğedir.
                        Err(e) => Outcome { path: raw.clone(), action: Action::Failed, bytes: 0, detail: Some(e) },
                    }
                }
            }
        };
        journal.record(&cat_name, &outcome)?;
        if matches!(outcome.action, Action::Trashed | Action::Deleted) {
            report.freed_bytes += outcome.bytes;
        }
        report.outcomes.push(outcome);
    }
    Ok(report)
}

pub fn clean(cat: Category, paths: &[String], permanent: bool, journal: &Journal) -> Result<Report, String> {
    let policy = Policy::from_env(&scan::roots_for(cat));
    run_with(cat, paths, permanent, &policy, &SystemRemover, journal)
}

/// Geri Dönüşüm'e taşınmış öğeleri özgün yerlerine geri yükler. Kalıcı silinenler geri alınamaz.
pub fn undo_with(paths: &[String], remover: &dyn Remover, journal: &Journal) -> Result<Report, String> {
    let mut report = Report { outcomes: Vec::new(), freed_bytes: 0 };
    for raw in paths {
        let outcome = match remover.restore(Path::new(raw)) {
            Ok(()) => Outcome { path: raw.clone(), action: Action::Restored, bytes: 0, detail: None },
            Err(e) => Outcome { path: raw.clone(), action: Action::Failed, bytes: 0, detail: Some(e) },
        };
        journal.record("undo", &outcome)?;
        report.outcomes.push(outcome);
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[derive(Default)]
    struct Fake {
        trashed: RefCell<Vec<PathBuf>>,
        removed: RefCell<Vec<PathBuf>>,
        restored: RefCell<Vec<PathBuf>>,
        fail: bool,
    }
    impl Remover for Fake {
        fn trash(&self, p: &Path) -> Result<(), String> {
            if self.fail { return Err("in use".into()); }
            self.trashed.borrow_mut().push(p.to_path_buf());
            Ok(())
        }
        fn remove_permanent(&self, p: &Path) -> Result<(), String> {
            self.removed.borrow_mut().push(p.to_path_buf());
            Ok(())
        }
        fn restore(&self, p: &Path) -> Result<(), String> {
            self.restored.borrow_mut().push(p.to_path_buf());
            Ok(())
        }
    }

    struct Sandbox(PathBuf);
    impl Sandbox {
        fn new(name: &str) -> Self {
            let p = std::env::temp_dir().join(format!("disktemizle_clean_{name}_{}", std::process::id()));
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
        fn journal(&self) -> Journal {
            Journal::open(self.0.join("journal/log.jsonl")).unwrap()
        }
        fn log(&self) -> String {
            fs::read_to_string(self.0.join("journal/log.jsonl")).unwrap()
        }
    }
    impl Drop for Sandbox {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn s(p: &Path) -> String { p.to_string_lossy().into_owned() }

    #[test]
    fn default_is_trash_never_permanent() {
        let sb = Sandbox::new("trash");
        let f = sb.file("tmp/a.bin", 10);
        let policy = Policy::new(&[], &[], &[sb.0.join("tmp")]);
        let fake = Fake::default();
        let r = run_with(Category::Temp, &[s(&f)], false, &policy, &fake, &sb.journal()).unwrap();
        assert_eq!(r.outcomes[0].action, Action::Trashed);
        assert_eq!(r.freed_bytes, 10);
        assert_eq!(fake.trashed.borrow().len(), 1);
        assert!(fake.removed.borrow().is_empty());
        assert!(sb.log().contains("\"trashed\""));
    }

    #[test]
    fn permanent_never_applies_to_dev_artifacts() {
        let sb = Sandbox::new("perm");
        sb.file("p/package.json", 1);
        let nm = sb.file("p/node_modules/x.js", 4).parent().unwrap().to_path_buf();
        let policy = Policy::new(&[], &[], &[sb.0.clone()]);
        let fake = Fake::default();
        let r = run_with(Category::DevArtifacts, &[s(&nm)], true, &policy, &fake, &sb.journal()).unwrap();
        assert_eq!(r.outcomes[0].action, Action::Trashed);
        assert!(fake.removed.borrow().is_empty());
    }

    #[test]
    fn user_files_are_never_deleted_permanently_and_must_be_files() {
        let sb = Sandbox::new("userf");
        let f = sb.file("home/Documents/big.iso", 10);
        let dir = sb.file("home/Documents/folder/x.bin", 1).parent().unwrap().to_path_buf();
        let policy = Policy::new(&[], &[], &[sb.0.join("home")]);
        let fake = Fake::default();
        let r = run_with(Category::LargeFiles, &[s(&f), s(&dir)], true, &policy, &fake, &sb.journal()).unwrap();
        assert_eq!(r.outcomes[0].action, Action::Trashed);
        assert_eq!(r.outcomes[1].action, Action::Skipped); // klasör: dosya değil
        assert!(fake.removed.borrow().is_empty());
        assert_eq!(fake.trashed.borrow().len(), 1);
    }

    #[test]
    fn duplicates_without_scan_record_are_skipped() {
        let sb = Sandbox::new("dupnr");
        let f = sb.file("home/Documents/copy.bin", 10);
        let policy = Policy::new(&[], &[], &[sb.0.join("home")]);
        let fake = Fake::default();
        let r = run_with(Category::Duplicates, &[s(&f)], false, &policy, &fake, &sb.journal()).unwrap();
        assert_eq!(r.outcomes[0].action, Action::Skipped);
        assert!(fake.trashed.borrow().is_empty());
    }

    #[test]
    fn permanent_allowed_for_caches() {
        let sb = Sandbox::new("cache");
        let f = sb.file("tmp/a.bin", 3);
        let policy = Policy::new(&[], &[], &[sb.0.join("tmp")]);
        let fake = Fake::default();
        let r = run_with(Category::Temp, &[s(&f)], true, &policy, &fake, &sb.journal()).unwrap();
        assert_eq!(r.outcomes[0].action, Action::Deleted);
    }

    #[test]
    fn protected_and_outside_paths_are_skipped() {
        let sb = Sandbox::new("skip");
        let win = sb.file("Windows/System32/x.dll", 1);
        let outside = sb.file("elsewhere/y.bin", 1);
        sb.file("tmp/keep.bin", 1);
        let policy = Policy::new(&[sb.0.join("Windows")], &[], &[sb.0.join("tmp")]);
        let fake = Fake::default();
        let r = run_with(Category::Temp, &[s(&win), s(&outside)], false, &policy, &fake, &sb.journal()).unwrap();
        assert!(r.outcomes.iter().all(|o| o.action == Action::Skipped));
        assert!(fake.trashed.borrow().is_empty());
        assert_eq!(r.freed_bytes, 0);
    }

    #[test]
    fn dev_artifact_without_marker_is_skipped() {
        let sb = Sandbox::new("marker");
        let t = sb.file("p3/target/keep.txt", 1).parent().unwrap().to_path_buf();
        let policy = Policy::new(&[], &[], &[sb.0.clone()]);
        let fake = Fake::default();
        let r = run_with(Category::DevArtifacts, &[s(&t)], false, &policy, &fake, &sb.journal()).unwrap();
        assert_eq!(r.outcomes[0].action, Action::Skipped);
        assert!(fake.trashed.borrow().is_empty());
    }

    #[test]
    fn locked_file_is_failed_not_fatal() {
        let sb = Sandbox::new("lock");
        let a = sb.file("tmp/a.bin", 1);
        let b = sb.file("tmp/b.bin", 1);
        let policy = Policy::new(&[], &[], &[sb.0.join("tmp")]);
        let fake = Fake { fail: true, ..Default::default() };
        let r = run_with(Category::Temp, &[s(&a), s(&b)], false, &policy, &fake, &sb.journal()).unwrap();
        assert_eq!(r.outcomes.len(), 2);
        assert!(r.outcomes.iter().all(|o| o.action == Action::Failed));
        assert_eq!(r.freed_bytes, 0);
    }

    #[test]
    fn journal_failure_aborts_before_next_item() {
        let sb = Sandbox::new("jfail");
        let a = sb.file("tmp/a.bin", 1);
        let policy = Policy::new(&[], &[], &[sb.0.join("tmp")]);
        let journal = Journal(sb.0.join("missing_dir/log.jsonl")); // yazılamaz
        assert!(run_with(Category::Temp, &[s(&a)], false, &policy, &Fake::default(), &journal).is_err());
    }

    /// Gerçek Geri Dönüşüm Kutusu: yalnızca bu testin oluşturduğu geçici dosyaya dokunur.
    /// `cargo test -- --ignored` ile çalıştırılır.
    #[test]
    #[ignore]
    fn real_recycle_bin_roundtrip() {
        let sb = Sandbox::new("real");
        let f = sb.file("tmp/roundtrip_marker.bin", 7);
        let policy = Policy::new(&[], &[], &[sb.0.join("tmp")]);
        let r = run_with(Category::Temp, &[s(&f)], false, &policy, &SystemRemover, &sb.journal()).unwrap();
        assert_eq!(r.outcomes[0].action, Action::Trashed, "{:?}", r.outcomes[0]);
        assert!(!f.exists());
        let u = undo_with(&[r.outcomes[0].path.clone()], &SystemRemover, &sb.journal()).unwrap();
        assert_eq!(u.outcomes[0].action, Action::Restored, "{:?}", u.outcomes[0]);
        assert!(f.exists());
    }

    #[test]
    fn journal_read_returns_newest_first_with_snake_case_category() {
        let sb = Sandbox::new("jread");
        let j = sb.journal();
        let a = sb.file("tmp/a.bin", 3);
        let b = sb.file("tmp/b.bin", 4);
        let policy = Policy::new(&[], &[], &[sb.0.join("tmp")]);
        run_with(Category::DevCache, &[s(&a)], false, &policy, &Fake::default(), &j).unwrap();
        run_with(Category::Temp, &[s(&b)], false, &policy, &Fake::default(), &j).unwrap();

        let e = j.read(10).unwrap();
        assert_eq!(e.len(), 2);
        assert_eq!(e[0].category, "temp");
        assert_eq!(e[1].category, "dev_cache");
        assert_eq!(e[0].action, "trashed");
        assert!(e.iter().all(|x| x.undoable));
        assert_eq!(j.read(1).unwrap().len(), 1);
    }

    #[test]
    fn restored_entries_are_no_longer_undoable_and_garbage_is_skipped() {
        let sb = Sandbox::new("jundo");
        let j = sb.journal();
        let a = sb.file("tmp/a.bin", 3);
        let policy = Policy::new(&[], &[], &[sb.0.join("tmp")]);
        let r = run_with(Category::Temp, &[s(&a)], false, &policy, &Fake::default(), &j).unwrap();
        undo_with(&[r.outcomes[0].path.clone()], &Fake::default(), &j).unwrap();
        // Bozuk satır eklenir.
        let mut f = OpenOptions::new().append(true).open(sb.0.join("journal/log.jsonl")).unwrap();
        writeln!(f, "not json at all").unwrap();

        let e = j.read(10).unwrap();
        assert_eq!(e.len(), 2);
        assert_eq!(e[0].action, "restored");
        assert!(e.iter().all(|x| !x.undoable), "{e:?}");
    }

    #[test]
    fn snake_case_conversion() {
        assert_eq!(snake_case("DevArtifacts"), "dev_artifacts");
        assert_eq!(snake_case("LargeFiles"), "large_files");
        assert_eq!(snake_case("Temp"), "temp");
        assert_eq!(snake_case("undo"), "undo");
    }

    #[test]
    fn undo_restores_and_logs() {
        let sb = Sandbox::new("undo");
        let fake = Fake::default();
        let r = undo_with(&["C:\\x\\a.bin".into()], &fake, &sb.journal()).unwrap();
        assert_eq!(r.outcomes[0].action, Action::Restored);
        assert_eq!(fake.restored.borrow().len(), 1);
        assert!(sb.log().contains("\"restored\""));
    }
}
