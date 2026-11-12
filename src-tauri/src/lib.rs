pub mod cleanup;
pub mod disk;
pub mod finder;
pub mod safety;
pub mod scan;

use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{Emitter, Manager};

fn open_journal(app: &tauri::AppHandle) -> Result<cleanup::Journal, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    cleanup::Journal::open(dir.join("journal.jsonl"))
}

/// İşlem günlüğü: en yeni kayıt başta. `undoable` olanlar Geri Dönüşüm'den geri yüklenebilir.
#[tauri::command]
fn journal_entries(app: tauri::AppHandle, limit: Option<usize>) -> Result<Vec<cleanup::Entry>, String> {
    open_journal(&app)?.read(limit.unwrap_or(500).min(5000))
}

#[tauri::command]
fn disk_info() -> Result<disk::DiskInfo, String> {
    disk::system_disk()
}

/// Varsayılan: Geri Dönüşüm'e taşır. `permanent` yalnızca önbellek kategorilerinde geçerlidir.
#[tauri::command]
async fn clean_items(
    app: tauri::AppHandle,
    id: String,
    paths: Vec<String>,
    permanent: bool,
) -> Result<cleanup::Report, String> {
    let cat = scan::Category::parse(&id).ok_or_else(|| format!("unknown category: {id}"))?;
    let journal = open_journal(&app)?;
    tauri::async_runtime::spawn_blocking(move || cleanup::clean(cat, &paths, permanent, &journal))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn undo_clean(app: tauri::AppHandle, paths: Vec<String>) -> Result<cleanup::Report, String> {
    let journal = open_journal(&app)?;
    tauri::async_runtime::spawn_blocking(move || cleanup::undo_with(&paths, &cleanup::SystemRemover, &journal))
        .await
        .map_err(|e| e.to_string())?
}

/// Salt-okunur: yalnızca aday listesi döndürür, hiçbir şeyi silmez.
/// Kategori başına bir iptal bayrağı; pencere kapanınca `cancel_scan` ile kurulur.
static CANCEL: [AtomicBool; 6] = [const { AtomicBool::new(false) }; 6];

fn cancel_flag(cat: scan::Category) -> &'static AtomicBool {
    &CANCEL[match cat {
        scan::Category::Temp => 0,
        scan::Category::Browser => 1,
        scan::Category::DevCache => 2,
        scan::Category::DevArtifacts => 3,
        scan::Category::LargeFiles => 4,
        scan::Category::Duplicates => 5,
    }]
}

/// `None` = tarama iptal edildi. İlerleme `scan-progress` olayıyla bildirilir.
#[tauri::command]
async fn scan_category(app: tauri::AppHandle, id: String) -> Result<Option<scan::ScanResult>, String> {
    let cat = scan::Category::parse(&id).ok_or_else(|| format!("unknown category: {id}"))?;
    let flag = cancel_flag(cat);
    flag.store(false, Ordering::Relaxed);
    tauri::async_runtime::spawn_blocking(move || {
        scan::scan(cat, flag, &|p| {
            let _ = app.emit("scan-progress", p);
        })
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
fn cancel_scan(id: String) {
    if let Some(cat) = scan::Category::parse(&id) {
        cancel_flag(cat).store(true, Ordering::Relaxed);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![scan_category, cancel_scan, clean_items, undo_clean, disk_info, journal_entries])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
