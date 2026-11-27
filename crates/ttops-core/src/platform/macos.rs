use std::path::{Path, PathBuf};

use super::{Platform, running_processes_sysinfo, volume_for};
use crate::model::{StartupScope, VolumeSummary};

pub struct MacOs {
    home: PathBuf,
}

impl MacOs {
    pub fn new() -> Option<Self> {
        Some(Self {
            home: dirs::home_dir()?,
        })
    }
}

impl Platform for MacOs {
    fn home(&self) -> PathBuf {
        self.home.clone()
    }
    fn cache_dir(&self) -> PathBuf {
        self.home.join("Library/Caches")
    }
    fn data_dir(&self) -> PathBuf {
        self.home.join("Library/Application Support")
    }
    fn local_data_dir(&self) -> PathBuf {
        self.data_dir()
    }
    fn env_var(&self, name: &str) -> Option<String> {
        std::env::var(name).ok()
    }
    fn protected_roots(&self) -> Vec<PathBuf> {
        let h = &self.home;
        let mut roots: Vec<PathBuf> = [
            "/System",
            "/bin",
            "/sbin",
            "/usr",
            "/etc",
            "/private/etc",
            "/var",
            "/private/var",
            "/Applications",
            "/Library",
        ]
        .iter()
        .map(PathBuf::from)
        .collect();
        roots.extend(
            [
                "Applications",
                "Library/Keychains",
                "Library/Mail",
                "Library/Messages",
                ".ssh",
                ".gnupg",
                ".aws",
                ".azure",
                ".kube",
                ".config/gcloud",
                ".docker/config.json",
            ]
            .iter()
            .map(|p| h.join(p)),
        );
        roots
    }
    fn catalog_source(&self) -> &'static str {
        include_str!("../../rules/macos.toml")
    }
    fn move_to_trash(&self, paths: &[PathBuf]) -> Result<(), trash::Error> {
        trash::delete_all(paths)
    }
    fn running_processes(&self) -> Vec<String> {
        running_processes_sysinfo()
    }
    fn app_dirs(&self) -> Vec<PathBuf> {
        vec![
            PathBuf::from("/Applications"),
            self.home.join("Applications"),
        ]
    }
    fn library_dir(&self) -> Option<PathBuf> {
        Some(self.home.join("Library"))
    }
    fn launch_dirs(&self) -> Vec<(PathBuf, StartupScope)> {
        vec![
            (
                self.home.join("Library/LaunchAgents"),
                StartupScope::UserAgent,
            ),
            (
                PathBuf::from("/Library/LaunchAgents"),
                StartupScope::SystemAgent,
            ),
            (
                PathBuf::from("/Library/LaunchDaemons"),
                StartupScope::SystemDaemon,
            ),
        ]
    }
    fn read_plist(&self, path: &Path) -> Option<serde_json::Value> {
        let output = std::process::Command::new("/usr/bin/plutil")
            .args(["-convert", "json", "-o", "-", "--"])
            .arg(path)
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        serde_json::from_slice(&output.stdout).ok()
    }
    fn user_id(&self) -> Option<u32> {
        use std::os::unix::fs::MetadataExt;
        std::fs::metadata(&self.home).ok().map(|m| m.uid())
    }
    fn volume_summary(&self) -> Option<VolumeSummary> {
        volume_for(&self.home, Some(Path::new("/System/Volumes/Data")))
    }
}
