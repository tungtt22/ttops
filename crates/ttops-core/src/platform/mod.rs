//! Platform abstraction layer (constitution Principle II). All OS-specific paths and
//! side effects go through [`Platform`] so tests can use [`TestPlatform`] rooted in a temp dir.

use std::io;
use std::path::{Path, PathBuf};

use crate::model::{BundleInfo, StartupScope, VolumeSummary};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

pub trait Platform: Send + Sync {
    fn home(&self) -> PathBuf;
    /// `{cache}`: ~/Library/Caches on macOS, %LOCALAPPDATA% on Windows.
    fn cache_dir(&self) -> PathBuf;
    /// `{data}`: ~/Library/Application Support on macOS, %APPDATA% on Windows.
    fn data_dir(&self) -> PathBuf;
    /// `{local_data}`: ~/Library/Application Support on macOS, %LOCALAPPDATA% on Windows.
    fn local_data_dir(&self) -> PathBuf;
    fn env_var(&self, name: &str) -> Option<String>;
    /// Paths that must never be deleted, nor any path inside them or containing them.
    fn protected_roots(&self) -> Vec<PathBuf>;
    /// TOML rule catalog for this OS.
    fn catalog_source(&self) -> &'static str;
    fn move_to_trash(&self, paths: &[PathBuf]) -> Result<(), trash::Error>;
    /// Size of the Recycle Bin when it is not a plain directory (Windows).
    fn recycle_bin_size(&self) -> Option<u64> {
        None
    }
    fn empty_recycle_bin(&self) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "no recycle bin API",
        ))
    }
    /// Lower-cased names of running processes.
    fn running_processes(&self) -> Vec<String>;
    fn volume_summary(&self) -> Option<VolumeSummary>;
    /// Folders whose direct children are uninstallable application bundles.
    fn app_dirs(&self) -> Vec<PathBuf> {
        Vec::new()
    }
    /// The user's Library folder (macOS) holding app leftovers.
    fn library_dir(&self) -> Option<PathBuf> {
        None
    }
    /// Startup item folders and their scope.
    fn launch_dirs(&self) -> Vec<(PathBuf, StartupScope)> {
        Vec::new()
    }
    /// Reads a property list as JSON; `None` when unreadable.
    fn read_plist(&self, _path: &Path) -> Option<serde_json::Value> {
        None
    }
    /// Bundle id, display name and executable of an application bundle.
    fn bundle_info(&self, app: &Path) -> Option<BundleInfo> {
        let info = self.read_plist(&app.join("Contents/Info.plist"))?;
        let text = |key: &str| info.get(key).and_then(|v| v.as_str()).map(str::to_owned);
        let bundle_id = text("CFBundleIdentifier")?;
        let fallback = app
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default();
        Some(BundleInfo {
            name: text("CFBundleName").unwrap_or_else(|| fallback.clone()),
            executable: text("CFBundleExecutable").unwrap_or(fallback),
            bundle_id,
        })
    }
    /// Numeric id of the current user, used for `gui/<uid>` launchd domains.
    fn user_id(&self) -> Option<u32> {
        None
    }
}

/// Platform for the OS this binary was built for. `None` means the home
/// directory could not be resolved at runtime.
pub fn native() -> Option<Box<dyn Platform>> {
    #[cfg(target_os = "macos")]
    {
        Some(Box::new(macos::MacOs::new()?))
    }
    #[cfg(target_os = "windows")]
    {
        Some(Box::new(windows::Windows::new()?))
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        None
    }
}

pub(crate) fn running_processes_sysinfo() -> Vec<String> {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};
    let Ok(mut sys) = System::new() else {
        return Vec::new();
    };
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
    sys.processes()
        .values()
        .map(|p| p.name().to_string_lossy().to_lowercase())
        .collect()
}

/// Volume holding `home`: the disk with the longest mount point prefix, preferring
/// `preferred` when present (APFS data volume on macOS).
pub(crate) fn volume_for(home: &Path, preferred: Option<&Path>) -> Option<VolumeSummary> {
    let disks = sysinfo::Disks::new_with_refreshed_list().ok()?;
    let disk_list: &[sysinfo::Disk] = &disks;
    let pick = preferred
        .and_then(|p| disk_list.iter().find(|d| d.mount_point() == p))
        .or_else(|| {
            disk_list
                .iter()
                .filter(|d| home.starts_with(d.mount_point()))
                .max_by_key(|d| d.mount_point().as_os_str().len())
        })?;
    let total = pick.total_space();
    let free = pick.available_space();
    Some(VolumeSummary {
        mount_point: pick.mount_point().to_path_buf(),
        total_bytes: total,
        used_bytes: total.saturating_sub(free),
        free_bytes: free,
    })
}

/// Test double: every location lives under `root`; "trash" moves into `root/.test-trash`.
pub struct TestPlatform {
    pub root: PathBuf,
    pub catalog: &'static str,
    pub processes: Vec<String>,
    pub extra_protected: Vec<PathBuf>,
}

impl TestPlatform {
    pub fn new(root: &Path, catalog: &'static str) -> Self {
        let p = Self {
            root: root.to_path_buf(),
            catalog,
            processes: Vec::new(),
            extra_protected: Vec::new(),
        };
        for d in [p.home(), p.cache_dir(), p.data_dir(), p.trash_dir()] {
            std::fs::create_dir_all(d).expect("create test dirs");
        }
        p
    }

    pub fn trash_dir(&self) -> PathBuf {
        self.root.join(".test-trash")
    }
}

impl Platform for TestPlatform {
    fn home(&self) -> PathBuf {
        self.root.join("home")
    }
    fn cache_dir(&self) -> PathBuf {
        self.home().join("cache")
    }
    fn data_dir(&self) -> PathBuf {
        self.home().join("data")
    }
    fn local_data_dir(&self) -> PathBuf {
        self.data_dir()
    }
    fn env_var(&self, name: &str) -> Option<String> {
        (name == "TTOPS_TEST").then(|| self.home().join("env").to_string_lossy().into_owned())
    }
    fn protected_roots(&self) -> Vec<PathBuf> {
        let mut roots = vec![
            self.root.join("system"),
            self.root.join("Applications"),
            self.home().join(".ssh"),
        ];
        roots.extend(self.extra_protected.iter().cloned());
        roots
    }
    fn catalog_source(&self) -> &'static str {
        self.catalog
    }
    fn move_to_trash(&self, paths: &[PathBuf]) -> Result<(), trash::Error> {
        for p in paths {
            let name = p.file_name().unwrap_or_default();
            let mut dest = self.trash_dir().join(name);
            let mut n = 1;
            while dest.exists() {
                dest = self
                    .trash_dir()
                    .join(format!("{}-{n}", name.to_string_lossy()));
                n += 1;
            }
            std::fs::rename(p, &dest).map_err(|e| trash::Error::Unknown {
                description: e.to_string(),
            })?;
        }
        Ok(())
    }
    fn running_processes(&self) -> Vec<String> {
        self.processes.clone()
    }
    fn app_dirs(&self) -> Vec<PathBuf> {
        vec![self.root.join("Applications")]
    }
    fn library_dir(&self) -> Option<PathBuf> {
        Some(self.home().join("Library"))
    }
    fn launch_dirs(&self) -> Vec<(PathBuf, StartupScope)> {
        vec![
            (
                self.home().join("Library/LaunchAgents"),
                StartupScope::UserAgent,
            ),
            (self.root.join("LaunchDaemons"), StartupScope::SystemDaemon),
        ]
    }
    /// Test plists are plain JSON files.
    fn read_plist(&self, path: &Path) -> Option<serde_json::Value> {
        serde_json::from_slice(&std::fs::read(path).ok()?).ok()
    }
    fn user_id(&self) -> Option<u32> {
        Some(501)
    }
    fn volume_summary(&self) -> Option<VolumeSummary> {
        Some(VolumeSummary {
            mount_point: self.root.clone(),
            total_bytes: 100,
            used_bytes: 60,
            free_bytes: 40,
        })
    }
}
