use std::path::{Path, PathBuf};

use super::{Platform, running_processes_sysinfo, volume_for};
use crate::model::VolumeSummary;

pub struct MacOs {
    home: PathBuf,
}

impl MacOs {
    pub fn new() -> Self {
        Self {
            home: dirs::home_dir().unwrap_or_else(|| PathBuf::from("/var/empty")),
        }
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
    fn volume_summary(&self) -> Option<VolumeSummary> {
        volume_for(&self.home, Some(Path::new("/System/Volumes/Data")))
    }
}
