//! Platform abstraction layer (constitution Principle II). All OS-specific paths and
//! side effects go through [`Platform`] so tests can use [`TestPlatform`] rooted in a temp dir.

use std::io;
use std::path::{Path, PathBuf};

use crate::model::VolumeSummary;

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
        Err(io::Error::new(io::ErrorKind::Unsupported, "no recycle bin API"))
    }
    /// Lower-cased names of running processes.
    fn running_processes(&self) -> Vec<String>;
    fn volume_summary(&self) -> Option<VolumeSummary>;
}

/// Platform for the OS this binary was built for.
pub fn native() -> Box<dyn Platform> {
    #[cfg(target_os = "macos")]
    {
        Box::new(macos::MacOs::new())
    }
    #[cfg(target_os = "windows")]
    {
        Box::new(windows::Windows::new())
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        compile_error!("TTops supports macOS and Windows only")
    }
}

pub(crate) fn running_processes_sysinfo() -> Vec<String> {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};
    let mut sys = System::new();
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
    sys.processes()
        .values()
        .map(|p| p.name().to_string_lossy().to_lowercase())
        .collect()
}

/// Volume holding `home`: the disk with the longest mount point prefix, preferring
/// `preferred` when present (APFS data volume on macOS).
pub(crate) fn volume_for(home: &Path, preferred: Option<&Path>) -> Option<VolumeSummary> {
    let disks = sysinfo::Disks::new_with_refreshed_list();
    let pick = preferred
        .and_then(|p| disks.iter().find(|d| d.mount_point() == p))
        .or_else(|| {
            disks
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
        let mut roots = vec![self.root.join("system"), self.home().join(".ssh")];
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
                dest = self.trash_dir().join(format!("{}-{n}", name.to_string_lossy()));
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
    fn volume_summary(&self) -> Option<VolumeSummary> {
        Some(VolumeSummary {
            mount_point: self.root.clone(),
            total_bytes: 100,
            used_bytes: 60,
            free_bytes: 40,
        })
    }
}
