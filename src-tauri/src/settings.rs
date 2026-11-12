//! Kalıcı ayarlar: hariç tutulan klasörler. Liste Rust tarafında tutulur; tarama onlara girmez ve
//! güvenlik kapısı (`safety::Policy`) onların içindeki ya da onları içeren hiçbir yolu silmez.

use crate::safety::{components, plain};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

const MAX_EXCLUDED: usize = 200;
const FILE: &str = "settings.json";

#[derive(Default, Serialize, Deserialize)]
struct Settings {
    #[serde(default)]
    excluded: Vec<String>,
}

/// (yol, küçük harfli bileşenler): bileşenler yürüyüşlerde hızlı karşılaştırma içindir.
static EXCLUDED: RwLock<Vec<(PathBuf, Vec<String>)>> = RwLock::new(Vec::new());

/// Testler genel listeyi paylaştığından, onu değiştiren testler bu kilidi alır.
#[cfg(test)]
pub(crate) static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub fn excluded_paths() -> Vec<PathBuf> {
    EXCLUDED.read().map(|g| g.iter().map(|(p, _)| p.clone()).collect()).unwrap_or_default()
}

/// Yol hariç tutulan bir klasörün kendisi ya da içindeyse `true`. (Hariç klasörü *içeren* bir
/// üst klasör burada hariç sayılmaz: yürüyüşler kökten başlayabilmeli. Silme kapısı o yönü ayrıca denetler.)
pub fn is_excluded(p: &Path) -> bool {
    let Ok(g) = EXCLUDED.read() else { return false };
    if g.is_empty() {
        return false;
    }
    components(p).is_some_and(|c| g.iter().any(|(_, e)| c.starts_with(e)))
}

pub(crate) fn install(list: &[PathBuf]) {
    let v = list.iter().filter_map(|p| components(p).map(|c| (p.clone(), c))).collect();
    if let Ok(mut g) = EXCLUDED.write() {
        *g = v;
    }
}

/// Kullanıcı girdisini doğrular: mutlak, erişilebilir bir klasör olmalı; sürücü kökü olamaz.
/// Yinelenenler (büyük/küçük harf farkı dahil) birleştirilir. Sonuç sıralı ve sadeleştirilmiştir.
pub fn validate(paths: &[String]) -> Result<Vec<PathBuf>, String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for raw in paths {
        let raw = raw.trim();
        let pb = PathBuf::from(raw);
        if !pb.is_absolute() {
            return Err(format!("not an absolute path: {raw}"));
        }
        let real = fs::canonicalize(&pb).map_err(|_| format!("cannot access: {raw}"))?;
        if !real.is_dir() {
            return Err(format!("not a folder: {raw}"));
        }
        if components(&real).is_none_or(|c| c.len() <= 1) {
            return Err(format!("a drive root cannot be excluded: {raw}"));
        }
        let shown = plain(&real);
        if seen.insert(shown.to_lowercase()) {
            out.push(PathBuf::from(shown));
        }
    }
    if out.len() > MAX_EXCLUDED {
        return Err(format!("too many excluded folders (max {MAX_EXCLUDED})"));
    }
    out.sort_by_key(|p| p.to_string_lossy().to_lowercase());
    Ok(out)
}

/// Uygulama açılışında dosyadan yükler. Bozuk ya da eksik dosya boş liste demektir; kaydedilmiş
/// klasörler artık yoksa bile korunur (ör. çıkarılmış harici disk).
pub fn load(dir: &Path) -> Vec<PathBuf> {
    let list: Vec<PathBuf> = fs::read_to_string(dir.join(FILE))
        .ok()
        .and_then(|s| serde_json::from_str::<Settings>(&s).ok())
        .map(|s| s.excluded.into_iter().map(PathBuf::from).collect())
        .unwrap_or_default();
    install(&list);
    list
}

/// Doğrular, kaydeder ve etkinleştirir. Başarısız olursa önceki liste aynen kalır.
pub fn set(dir: &Path, paths: &[String]) -> Result<Vec<PathBuf>, String> {
    let list = validate(paths)?;
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let body = serde_json::to_string_pretty(&Settings { excluded: list.iter().map(|p| plain(p)).collect() })
        .map_err(|e| e.to_string())?;
    fs::write(dir.join(FILE), body).map_err(|e| e.to_string())?;
    install(&list);
    Ok(list)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Sandbox(PathBuf);
    impl Sandbox {
        fn new(name: &str) -> Self {
            let p = std::env::temp_dir().join(format!("disktemizle_set_{name}_{}", std::process::id()));
            let _ = fs::remove_dir_all(&p);
            fs::create_dir_all(&p).unwrap();
            Self(fs::canonicalize(p).unwrap())
        }
        fn dir(&self, rel: &str) -> PathBuf {
            let p = self.0.join(rel);
            fs::create_dir_all(&p).unwrap();
            fs::canonicalize(p).unwrap()
        }
    }
    impl Drop for Sandbox {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn s(p: &Path) -> String {
        plain(p)
    }

    /// Gerçek profilde salt-okunur: `Downloads` hariç tutulunca büyük dosya listesi ve silme kapısı nasıl değişir.
    /// `cargo test -- --ignored --nocapture real_exclusion`.
    #[test]
    #[ignore]
    fn real_exclusion_effect() {
        use crate::safety::{Mode, Policy};
        use crate::scan::{roots_for, scan, Category};
        use std::sync::atomic::AtomicBool;
        let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let downloads = PathBuf::from(std::env::var_os("USERPROFILE").unwrap()).join("Downloads");
        let run = || scan(Category::LargeFiles, &AtomicBool::new(false), &|_| {}).unwrap();

        install(&[]);
        let before = run();
        let in_dl = |r: &crate::scan::ScanResult| r.items.iter().filter(|i| i.path.contains("\\Downloads\\")).count();
        println!("before: {} items, {} in Downloads", before.items.len(), in_dl(&before));
        let sample = before.items.iter().find(|i| i.path.contains("\\Downloads\\")).map(|i| i.path.clone());

        install(&[downloads.clone()]);
        let after = run();
        println!("after : {} items, {} in Downloads", after.items.len(), in_dl(&after));
        assert_eq!(in_dl(&after), 0);

        if let Some(p) = sample {
            let policy = Policy::from_env(&roots_for(Category::LargeFiles));
            let res = policy.validate(Path::new(&p), Mode::UserFile);
            println!("validate({p}) -> {res:?}");
            assert!(res.is_err());
        }
        install(&[]);
    }

    #[test]
    fn validate_rejects_bad_input() {
        let sb = Sandbox::new("val");
        let file = sb.0.join("f.txt");
        fs::write(&file, b"x").unwrap();
        assert!(validate(&["relative\\path".into()]).is_err());
        assert!(validate(&[s(&sb.0.join("missing"))]).is_err());
        assert!(validate(&[s(&file)]).is_err());
        assert!(validate(&["C:\\".into()]).is_err());
    }

    #[test]
    fn validate_normalises_and_dedupes() {
        let sb = Sandbox::new("dedupe");
        let a = sb.dir("Projeler");
        let upper = a.to_string_lossy().to_uppercase();
        let out = validate(&[s(&a), upper, format!("  {}  ", s(&a))]).unwrap();
        assert_eq!(out.len(), 1);
        assert!(!out[0].to_string_lossy().starts_with(r"\\?\"));
    }

    #[test]
    fn set_and_load_roundtrip_and_failure_keeps_previous_list() {
        let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let sb = Sandbox::new("round");
        let cfg = sb.dir("cfg");
        let keep = sb.dir("keep");

        let saved = set(&cfg, &[s(&keep)]).unwrap();
        assert_eq!(saved.len(), 1);
        assert!(is_excluded(&keep.join("anything").join("deep")));

        // Geçersiz girdi: hata verir, önceki liste ve dosya bozulmaz.
        assert!(set(&cfg, &["not\\absolute".into()]).is_err());
        assert!(is_excluded(&keep));

        install(&[]);
        assert!(!is_excluded(&keep));
        let loaded = load(&cfg);
        assert_eq!(loaded.len(), 1);
        assert!(is_excluded(&keep));
        install(&[]);
    }

    #[test]
    fn load_survives_missing_and_garbage_files() {
        let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let sb = Sandbox::new("garbage");
        assert!(load(&sb.0).is_empty());
        fs::write(sb.0.join(FILE), "{ not json").unwrap();
        assert!(load(&sb.0).is_empty());
        install(&[]);
    }

    #[test]
    fn exclusion_is_inside_or_equal_only_and_walkers_skip_it() {
        let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let sb = Sandbox::new("walk");
        let skip = sb.dir("skip");
        for p in ["proj", "skip"] {
            let d = sb.dir(p);
            fs::write(d.join("package.json"), b"{}").unwrap();
            let nm = sb.dir(&format!("{p}/node_modules"));
            fs::write(nm.join("x.js"), b"1").unwrap();
        }
        install(&[skip.clone()]);
        assert!(is_excluded(&skip));
        assert!(is_excluded(&skip.join("a")));
        assert!(!is_excluded(&sb.0), "parent of an excluded folder is not itself excluded");
        assert!(!is_excluded(&sb.0.join("proj")));

        let policy = crate::safety::Policy::new(&[], &[], &[sb.0.clone()]);
        let r = crate::scan::scan_roots(
            crate::scan::Category::DevArtifacts,
            &[sb.0.clone()],
            Some(&policy),
            0,
            &std::sync::atomic::AtomicBool::new(false),
            &|_| {},
        )
        .unwrap();
        let paths: Vec<_> = r.items.iter().map(|i| i.path.clone()).collect();
        assert_eq!(paths.len(), 1, "{paths:?}");
        assert!(paths[0].contains("proj"));
        install(&[]);
    }
}
