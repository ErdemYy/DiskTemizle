//! Geri Dönüşüm Kutusu: listeleme ve (yalnızca kullanıcı seçerse) kalıcı boşaltma.
//! Tarama salt-okunurdur. Boşaltma geri alınamaz; yalnızca kutudaki öğelere dokunur.

use crate::safety::plain;
use crate::scan::{measure, Category, Item, Phase, Progress, ProgressFn, ScanResult};
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering::Relaxed};
use std::time::{SystemTime, UNIX_EPOCH};
use trash::{os_limited, TrashItem, TrashItemSize};

const DAY: u64 = 24 * 3600;
/// Bundan eski öğeler "önerilen" sayılır: kullanıcı büyük olasılıkla artık onları istemiyor.
const OLD_AFTER_SECS: u64 = 30 * DAY;

/// Kutudaki öğenin özgün yolu, eşleştirme için sadeleştirilmiş ve küçük harfli.
fn key(item: &TrashItem) -> String {
    plain(&item.original_path()).to_lowercase()
}

fn age_secs(item: &TrashItem) -> u64 {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
    (now - item.time_deleted).max(0) as u64
}

/// (bayt, dosya sayısı). `id` çoğu zaman kutudaki gerçek `$R…` yoludur; değilse kabuk meta verisine düşer.
fn size_of(item: &TrashItem, cancel: &AtomicBool) -> (u64, u64) {
    let p = Path::new(&item.id);
    if p.exists() {
        let m = measure(p, cancel);
        return (m.size_bytes, m.file_count);
    }
    match os_limited::metadata(item) {
        Ok(m) => match m.size {
            TrashItemSize::Bytes(b) => (b, 1),
            TrashItemSize::Entries(n) => (0, n as u64),
        },
        Err(_) => (0, 0),
    }
}

pub fn scan(cancel: &AtomicBool, progress: ProgressFn) -> Option<ScanResult> {
    let entries = os_limited::list().unwrap_or_default();
    let total = entries.len() as u32;
    let done = AtomicU32::new(0);
    let step = (total / 100).max(1);
    progress(Progress { category: Category::RecycleBin, phase: Phase::Measure, done: 0, total });

    let mut items: Vec<Item> = entries
        .par_iter()
        .filter_map(|e| {
            if cancel.load(Relaxed) {
                return None;
            }
            let (bytes, files) = size_of(e, cancel);
            let age = age_secs(e);
            let mut item = Item::plain(plain(&e.original_path()), bytes, files, age);
            item.recommended = age >= OLD_AFTER_SECS;
            item.reason = Some("in_bin");
            let d = done.fetch_add(1, Relaxed) + 1;
            if d % step == 0 || d == total {
                progress(Progress { category: Category::RecycleBin, phase: Phase::Measure, done: d, total });
            }
            Some(item)
        })
        .collect();
    if cancel.load(Relaxed) {
        return None;
    }
    items.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes).then_with(|| a.path.cmp(&b.path)));
    let total_bytes = items.iter().map(|i| i.size_bytes).sum();
    Some(ScanResult { category: Category::RecycleBin, items, total_bytes, rejected: 0, hidden_recent: 0 })
}

/// Verilen özgün yollara karşılık gelen kutu öğelerini kalıcı siler. Sonuç sırası `paths` ile aynıdır:
/// `Ok(bayt)` ya da `Err(gerekçe)`. Kutu bir kez listelenir; toplu silme başarısız olursa
/// hatalı öğeyi ayırmak için yollar tek tek denenir.
pub fn purge(paths: &[String]) -> Vec<Result<u64, String>> {
    let cancel = AtomicBool::new(false);
    let all = match os_limited::list() {
        Ok(v) => v,
        Err(e) => return paths.iter().map(|_| Err(e.to_string())).collect(),
    };
    let mut by_key: HashMap<String, Vec<TrashItem>> = HashMap::new();
    for item in all {
        by_key.entry(key(&item)).or_default().push(item);
    }

    // Aynı yol listede birden fazla gelirse tek sefer silinir.
    let mut taken: HashMap<String, Vec<TrashItem>> = HashMap::new();
    let wanted: Vec<Option<(String, u64)>> = paths
        .iter()
        .map(|p| {
            let k = p.to_lowercase();
            if let Some(items) = by_key.remove(&k) {
                let bytes = items.iter().map(|i| size_of(i, &cancel).0).sum();
                taken.insert(k.clone(), items);
                Some((k, bytes))
            } else if taken.contains_key(&k) {
                Some((k, 0))
            } else {
                None
            }
        })
        .collect();

    let everything: Vec<TrashItem> = taken.values().flatten().cloned().collect();
    let bulk_ok = everything.is_empty() || os_limited::purge_all(everything).is_ok();

    wanted
        .into_iter()
        .map(|w| match w {
            None => Err("not in Recycle Bin".to_string()),
            Some((_, bytes)) if bulk_ok => Ok(bytes),
            Some((k, bytes)) => {
                // Toplu işlem başarısızdı: bu yolu tek başına dene.
                let items = taken.get(&k).cloned().unwrap_or_default();
                os_limited::purge_all(items).map(|_| bytes).map_err(|e| e.to_string())
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Gerçek kutuya yalnızca bu testin oluşturduğu geçici dosyayı koyar ve kendi eliyle kalıcı siler.
    /// `cargo test -- --ignored real_bin_roundtrip`.
    #[test]
    #[ignore]
    fn real_bin_roundtrip() {
        let dir = std::env::temp_dir().join(format!("disktemizle_bin_{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let f = dir.join("bin_roundtrip_marker.bin");
        fs::write(&f, vec![9u8; 12345]).unwrap();
        let canon = plain(&fs::canonicalize(&f).unwrap());
        trash::delete(&f).unwrap();

        let r = scan(&AtomicBool::new(false), &|_| {}).unwrap();
        let item = r.items.iter().find(|i| i.path.to_lowercase() == canon.to_lowercase()).expect("item in bin scan");
        assert_eq!(item.size_bytes, 12345, "size from bin entry");
        assert!(!item.recommended, "fresh item must not be recommended");

        let res = purge(&[canon.clone(), "C:\\never\\in\\bin.txt".into()]);
        assert_eq!(res[0], Ok(12345), "{res:?}");
        assert!(res[1].is_err());

        let again = scan(&AtomicBool::new(false), &|_| {}).unwrap();
        assert!(again.items.iter().all(|i| i.path.to_lowercase() != canon.to_lowercase()));
        let _ = fs::remove_dir_all(&dir);
    }

    /// Kendi Geri Dönüşüm Kutunuzun salt-okunur özeti. `cargo test --release -- --ignored --nocapture real_bin_report`.
    #[test]
    #[ignore]
    fn real_bin_report() {
        let t = std::time::Instant::now();
        let r = scan(&AtomicBool::new(false), &|_| {}).unwrap();
        println!(
            "RecycleBin: {} items, {:.1} MB, {} recommended (older than 30d), {:.1}s",
            r.items.len(),
            r.total_bytes as f64 / 1048576.0,
            r.items.iter().filter(|i| i.recommended).count(),
            t.elapsed().as_secs_f32()
        );
        for i in r.items.iter().take(6) {
            println!("   {:>9.1} MB  {:>5}d  {}", i.size_bytes as f64 / 1048576.0, i.age_secs / DAY, i.path);
        }
    }
}
