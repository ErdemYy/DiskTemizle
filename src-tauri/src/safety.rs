//! Silme öncesi güvenlik kapısı. Hiçbir silme/taşıma işlemi `Policy::validate`
//! başarılı dönmeden yapılmamalıdır.

use std::fmt;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, PartialEq, Eq)]
pub enum SafetyError {
    NotAbsolute,
    ParentTraversal,
    Unresolvable,
    DriveRoot,
    Protected(String),
    ContainsProtected(String),
    OutsideAllowedRoots,
    SensitiveComponent(String),
    NotDevArtifact,
}

impl fmt::Display for SafetyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAbsolute => write!(f, "path is not absolute"),
            Self::ParentTraversal => write!(f, "path contains '..'"),
            Self::Unresolvable => write!(f, "path cannot be resolved"),
            Self::DriveRoot => write!(f, "drive root is never deletable"),
            Self::Protected(p) => write!(f, "inside protected location: {p}"),
            Self::ContainsProtected(p) => write!(f, "contains protected location: {p}"),
            Self::OutsideAllowedRoots => write!(f, "outside the category's allowed roots"),
            Self::SensitiveComponent(c) => write!(f, "sensitive folder in path: {c}"),
            Self::NotDevArtifact => write!(f, "not a recognised developer artifact"),
        }
    }
}

/// Hangi kategori için doğrulama yapılıyor.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// Önbellek/geçici dosya: kullanıcı klasörlerinin (Belgeler, Masaüstü...) içi yasak.
    Strict,
    /// Geliştirici artığı: kullanıcı klasörlerinde yalnızca bilinen artık klasör adları serbest.
    DevArtifact,
    /// Kullanıcının kendi dosyası (büyük/yinelenen dosya bulucu): Belgeler, İndirilenler vb. serbest,
    /// ama sistem klasörleri, AppData, `.git`/`.ssh` yine korunur. Çağıran yalnızca dosya vermelidir.
    UserFile,
}

const DEV_ARTIFACT_NAMES: &[&str] = &[
    "node_modules", "__pycache__", "target", "dist", "build", ".next", ".gradle", ".pytest_cache",
];

/// Yolun her yerinde geçmesi yasak bileşenler.
const SENSITIVE_COMPONENTS: &[&str] = &[".git", ".ssh", ".gnupg", ".aws"];

pub struct Policy {
    /// Mutlak koruma: Mode ne olursa olsun içine girilemez.
    system_roots: Vec<Vec<String>>,
    /// Kullanıcı verisi: yalnızca Mode::DevArtifact istisnası olabilir.
    user_roots: Vec<Vec<String>>,
    /// Silinebilecek köklerin dışına çıkılamaz (kategoriye göre verilir).
    allowed_roots: Vec<Vec<String>>,
    /// Çalışan uygulamanın yürütülebilir dosyası: onu içeren hiçbir klasör silinemez
    /// (ör. geliştirme sırasında `target` klasörü).
    exe: Option<Vec<String>>,
    /// Uygulama verisi (AppData): `Mode::UserFile` için mutlak korumalı.
    app_roots: Vec<Vec<String>>,
}

impl Policy {
    pub fn new(system: &[PathBuf], user: &[PathBuf], allowed: &[PathBuf]) -> Self {
        let norm = |v: &[PathBuf]| v.iter().filter_map(|p| components(&resolve_lossy(p))).collect();
        let exe = std::env::current_exe().ok().and_then(|p| components(&resolve_lossy(&p)));
        Self {
            system_roots: norm(system),
            user_roots: norm(user),
            allowed_roots: norm(allowed),
            exe,
            app_roots: Vec::new(),
        }
    }

    pub fn with_app_roots(mut self, app: &[PathBuf]) -> Self {
        self.app_roots = app.iter().filter_map(|p| components(&resolve_lossy(p))).collect();
        self
    }

    /// Gerçek Windows ortamından korumalı yolları toplar.
    pub fn from_env(allowed: &[PathBuf]) -> Self {
        let mut system = Vec::new();
        for var in ["SystemRoot", "ProgramFiles", "ProgramFiles(x86)", "ProgramData", "SystemDrive"] {
            if let Some(v) = std::env::var_os(var) {
                let mut p = PathBuf::from(v);
                if var == "SystemDrive" {
                    p.push("\\");
                    p.push("Recovery");
                }
                system.push(p);
            }
        }
        let mut user = Vec::new();
        if let Some(home) = std::env::var_os("USERPROFILE").map(PathBuf::from) {
            for d in ["Documents", "Belgeler", "Desktop", "Masaüstü", "Pictures", "Resimler",
                      "Videos", "Music", "Downloads", "OneDrive"] {
                user.push(home.join(d));
            }
            user.push(home.join("AppData").join("Roaming"));
        }
        for var in ["OneDrive", "OneDriveConsumer", "OneDriveCommercial"] {
            if let Some(v) = std::env::var_os(var) {
                user.push(PathBuf::from(v));
            }
        }
        let app: Vec<PathBuf> = std::env::var_os("USERPROFILE")
            .map(|h| vec![PathBuf::from(h).join("AppData")])
            .unwrap_or_default();
        Self::new(&system, &user, allowed).with_app_roots(&app)
    }

    /// Doğrular ve silinecek kesin (çözümlenmiş) yolu döndürür.
    pub fn validate(&self, path: &Path, mode: Mode) -> Result<PathBuf, SafetyError> {
        if !path.is_absolute() {
            return Err(SafetyError::NotAbsolute);
        }
        // `\\?\` önekli yollarda `..` ParentDir değil düz bileşen sayılır; ikisi de reddedilir.
        if path.components().any(|c| match c {
            Component::ParentDir => true,
            Component::Normal(n) => n == "..",
            _ => false,
        }) {
            return Err(SafetyError::ParentTraversal);
        }
        // Sembolik bağlantı / junction burada çözülür; asıl hedef denetlenir.
        let real = std::fs::canonicalize(path).map_err(|_| SafetyError::Unresolvable)?;
        let comps = components(&real).ok_or(SafetyError::Unresolvable)?;

        if comps.len() <= 1 {
            return Err(SafetyError::DriveRoot);
        }
        if let Some(bad) = comps.iter().find(|c| SENSITIVE_COMPONENTS.contains(&c.as_str())) {
            return Err(SafetyError::SensitiveComponent(bad.clone()));
        }
        if self.exe.as_ref().is_some_and(|exe| exe.starts_with(&comps)) {
            return Err(SafetyError::ContainsProtected("running application".into()));
        }
        for root in &self.system_roots {
            self.check_root(&comps, root, true)?;
        }
        if mode == Mode::UserFile {
            // Kullanıcı içeriği serbest; yalnızca uygulama verisi korunur.
            for root in &self.app_roots {
                self.check_root(&comps, root, true)?;
            }
            if !self.allowed_roots.is_empty()
                && !self.allowed_roots.iter().any(|r| comps.len() > r.len() && comps.starts_with(r))
            {
                return Err(SafetyError::OutsideAllowedRoots);
            }
            return Ok(real);
        }
        let dev_exempt = mode == Mode::DevArtifact
            && comps.last().is_some_and(|n| DEV_ARTIFACT_NAMES.contains(&n.as_str()));
        if mode == Mode::DevArtifact && !dev_exempt {
            return Err(SafetyError::NotDevArtifact);
        }
        if !dev_exempt {
            for root in &self.user_roots {
                self.check_root(&comps, root, true)?;
            }
        } else {
            // İstisna içinde bile, korumalı klasörün kendisini veya üstünü silme.
            for root in &self.user_roots {
                if root.starts_with(&comps) {
                    return Err(SafetyError::ContainsProtected(root.join("\\")));
                }
            }
        }
        if !self.allowed_roots.is_empty()
            && !self.allowed_roots.iter().any(|r| comps.len() > r.len() && comps.starts_with(r))
        {
            return Err(SafetyError::OutsideAllowedRoots);
        }
        Ok(real)
    }

    /// `target` korumalı kökün içindeyse VEYA onu içeriyorsa (üst klasör) hata.
    fn check_root(&self, target: &[String], root: &[String], _hard: bool) -> Result<(), SafetyError> {
        if target.starts_with(root) {
            return Err(SafetyError::Protected(root.join("\\")));
        }
        if root.starts_with(target) {
            return Err(SafetyError::ContainsProtected(root.join("\\")));
        }
        Ok(())
    }
}

/// `canonicalize` çıktısındaki `\\?\` önekini atar (UNC: `\\?\UNC\sunucu` -> `\\sunucu`).
pub fn plain(p: &Path) -> String {
    let s = p.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{rest}");
    }
    s.strip_prefix(r"\\?\").unwrap_or(&s).to_string()
}

/// `\\?\` önekini atar, bileşenleri küçük harfe çevirir (Windows büyük/küçük harf duyarsız).
fn components(p: &Path) -> Option<Vec<String>> {
    let s = p.to_string_lossy();
    let s = s.strip_prefix(r"\\?\").unwrap_or(&s);
    let out: Vec<String> = Path::new(s)
        .components()
        .filter_map(|c| match c {
            Component::Prefix(p) => Some(p.as_os_str().to_string_lossy().to_lowercase()),
            Component::Normal(n) => Some(n.to_string_lossy().to_lowercase()),
            _ => None,
        })
        .collect();
    if out.is_empty() { None } else { Some(out) }
}

/// Korumalı kök henüz yoksa bile mümkün olduğunca çözümler.
fn resolve_lossy(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct Sandbox(PathBuf);
    impl Sandbox {
        fn new(name: &str) -> Self {
            let p = std::env::temp_dir().join(format!("disktemizle_test_{name}_{}", std::process::id()));
            let _ = fs::remove_dir_all(&p);
            fs::create_dir_all(&p).unwrap();
            Self(fs::canonicalize(p).unwrap())
        }
        fn dir(&self, rel: &str) -> PathBuf {
            let p = self.0.join(rel);
            fs::create_dir_all(&p).unwrap();
            fs::canonicalize(p).unwrap()
        }
        fn file_in(&self, rel: &str) -> PathBuf {
            let p = self.0.join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(&p, b"x").unwrap();
            fs::canonicalize(p).unwrap()
        }
    }
    impl Drop for Sandbox {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn policy(sb: &Sandbox, allowed: &[PathBuf]) -> Policy {
        Policy::new(&[sb.dir("Windows")], &[sb.dir("home/Documents"), sb.dir("home/Desktop")], allowed)
    }

    #[test]
    fn rejects_relative_and_traversal() {
        let sb = Sandbox::new("rel");
        let p = policy(&sb, &[]);
        assert_eq!(p.validate(Path::new("foo"), Mode::Strict), Err(SafetyError::NotAbsolute));
        // join() verbatim yollarda `..` ifadesini kendisi çözer; yolu elle kuruyoruz.
        let tricky = PathBuf::from(format!("{}\\..\\Windows", sb.dir("cache").display()));
        assert_eq!(p.validate(&tricky, Mode::Strict), Err(SafetyError::ParentTraversal));
    }

    #[test]
    fn rejects_folder_containing_running_exe() {
        let sb = Sandbox::new("exe");
        let p = policy(&sb, &[]);
        let exe_dir = std::env::current_exe().unwrap().parent().unwrap().to_path_buf();
        assert!(matches!(p.validate(&exe_dir, Mode::Strict), Err(SafetyError::ContainsProtected(_))));
        // Üst klasörü (ör. `target`) da reddedilir.
        let parent = exe_dir.parent().unwrap().to_path_buf();
        assert!(matches!(p.validate(&parent, Mode::DevArtifact), Err(SafetyError::ContainsProtected(_))));
    }

    #[test]
    fn user_file_mode_allows_documents_but_not_appdata_or_system() {
        let sb = Sandbox::new("userfile");
        let p = policy(&sb, &[]).with_app_roots(&[sb.dir("home/AppData")]);
        let doc = sb.file_in("home/Documents/big.iso");
        let appdata = sb.file_in("home/AppData/Local/x.bin");
        let sys = sb.file_in("Windows/System32/x.dll");
        let git = sb.file_in("home/Documents/repo/.git/pack.bin");
        assert!(p.validate(&doc, Mode::UserFile).is_ok());
        assert!(matches!(p.validate(&appdata, Mode::UserFile), Err(SafetyError::Protected(_))));
        assert!(matches!(p.validate(&sys, Mode::UserFile), Err(SafetyError::Protected(_))));
        assert!(matches!(p.validate(&git, Mode::UserFile), Err(SafetyError::SensitiveComponent(_))));
        // Aynı dosya Strict modda hâlâ korumalı (Belgeler).
        assert!(matches!(p.validate(&doc, Mode::Strict), Err(SafetyError::Protected(_))));
    }

    #[test]
    fn rejects_drive_root() {
        let sb = Sandbox::new("root");
        let p = policy(&sb, &[]);
        assert_eq!(p.validate(Path::new("C:\\"), Mode::Strict), Err(SafetyError::DriveRoot));
    }

    #[test]
    fn rejects_system_and_inside_system() {
        let sb = Sandbox::new("sys");
        let p = policy(&sb, &[]);
        let win = sb.dir("Windows");
        let inner = sb.dir("Windows/System32");
        assert!(matches!(p.validate(&win, Mode::Strict), Err(SafetyError::Protected(_))));
        assert!(matches!(p.validate(&inner, Mode::Strict), Err(SafetyError::Protected(_))));
        assert!(matches!(p.validate(&inner, Mode::DevArtifact), Err(SafetyError::Protected(_))));
    }

    #[test]
    fn rejects_parent_of_protected() {
        let sb = Sandbox::new("parent");
        let p = policy(&sb, &[]);
        let home = sb.dir("home");
        assert!(matches!(p.validate(&home, Mode::Strict), Err(SafetyError::ContainsProtected(_))));
    }

    #[test]
    fn rejects_user_folders_in_strict_mode() {
        let sb = Sandbox::new("user");
        let p = policy(&sb, &[]);
        let doc = sb.dir("home/Documents/projects/x");
        assert!(matches!(p.validate(&doc, Mode::Strict), Err(SafetyError::Protected(_))));
    }

    #[test]
    fn dev_artifact_allowed_in_documents_only_by_name() {
        let sb = Sandbox::new("dev");
        let p = policy(&sb, &[]);
        let nm = sb.dir("home/Documents/app/node_modules");
        let src = sb.dir("home/Documents/app/src");
        assert!(p.validate(&nm, Mode::DevArtifact).is_ok());
        assert_eq!(p.validate(&src, Mode::DevArtifact), Err(SafetyError::NotDevArtifact));
    }

    #[test]
    fn rejects_sensitive_components() {
        let sb = Sandbox::new("sens");
        let p = policy(&sb, &[]);
        let git = sb.dir("work/repo/.git/objects");
        assert!(matches!(p.validate(&git, Mode::Strict), Err(SafetyError::SensitiveComponent(_))));
        let ssh = sb.dir("work/.ssh");
        assert!(matches!(p.validate(&ssh, Mode::Strict), Err(SafetyError::SensitiveComponent(_))));
    }

    #[test]
    fn enforces_allowed_roots() {
        let sb = Sandbox::new("allow");
        let cache = sb.dir("cache");
        let other = sb.dir("other/stuff");
        let p = policy(&sb, &[cache.clone()]);
        let inside = sb.dir("cache/chrome");
        assert!(p.validate(&inside, Mode::Strict).is_ok());
        assert_eq!(p.validate(&other, Mode::Strict), Err(SafetyError::OutsideAllowedRoots));
        // Kökün kendisi silinemez, yalnızca içeriği.
        assert_eq!(p.validate(&cache, Mode::Strict), Err(SafetyError::OutsideAllowedRoots));
    }

    #[test]
    fn nonexistent_is_unresolvable() {
        let sb = Sandbox::new("nx");
        let p = policy(&sb, &[]);
        assert_eq!(p.validate(&sb.0.join("nope"), Mode::Strict), Err(SafetyError::Unresolvable));
    }

    #[test]
    fn case_insensitive_match() {
        let sb = Sandbox::new("case");
        let p = policy(&sb, &[]);
        let upper = PathBuf::from(sb.dir("Windows/System32").to_string_lossy().to_uppercase());
        assert!(matches!(p.validate(&upper, Mode::Strict), Err(SafetyError::Protected(_))));
    }
}
