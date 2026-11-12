//! Sistem sürücüsünün toplam/boş alanı (salt-okunur).

use serde::Serialize;
use std::path::Path;

#[derive(Debug, Serialize)]
pub struct DiskInfo {
    pub label: String,
    pub total_bytes: u64,
    pub free_bytes: u64,
}

pub fn system_disk() -> Result<DiskInfo, String> {
    let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
    let root = format!("{drive}\\");
    let path = Path::new(&root);
    Ok(DiskInfo {
        label: drive,
        total_bytes: fs2::total_space(path).map_err(|e| e.to_string())?,
        free_bytes: fs2::available_space(path).map_err(|e| e.to_string())?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_sane_values() {
        let d = system_disk().unwrap();
        assert!(d.total_bytes > 0);
        assert!(d.free_bytes <= d.total_bytes);
    }
}
