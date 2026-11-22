use std::io;
use std::path::PathBuf;

use super::{Platform, running_processes_sysinfo, volume_for};
use crate::model::VolumeSummary;

pub struct Windows {
    home: PathBuf,
}

impl Windows {
    pub fn new() -> Self {
        Self {
            home: dirs::home_dir().unwrap_or_else(|| PathBuf::from(r"C:\Users\Default")),
        }
    }

    fn env_path(&self, name: &str, fallback: &str) -> PathBuf {
        std::env::var_os(name)
            .map(PathBuf::from)
            .unwrap_or_else(|| self.home.join(fallback))
    }
}

impl Platform for Windows {
    fn home(&self) -> PathBuf {
        self.home.clone()
    }
    fn cache_dir(&self) -> PathBuf {
        self.local_data_dir()
    }
    fn data_dir(&self) -> PathBuf {
        self.env_path("APPDATA", r"AppData\Roaming")
    }
    fn local_data_dir(&self) -> PathBuf {
        self.env_path("LOCALAPPDATA", r"AppData\Local")
    }
    fn env_var(&self, name: &str) -> Option<String> {
        std::env::var(name).ok()
    }
    fn protected_roots(&self) -> Vec<PathBuf> {
        let mut roots = Vec::new();
        for var in [
            "SystemRoot",
            "ProgramFiles",
            "ProgramFiles(x86)",
            "ProgramW6432",
            "ProgramData",
        ] {
            if let Some(v) = std::env::var_os(var) {
                roots.push(PathBuf::from(v));
            }
        }
        let appdata = self.data_dir();
        let local = self.local_data_dir();
        roots.extend([
            appdata.join(r"Microsoft\Credentials"),
            appdata.join(r"Microsoft\Protect"),
            appdata.join(r"Microsoft\Crypto"),
            local.join(r"Microsoft\Credentials"),
            local.join(r"Microsoft\Vault"),
            self.home.join(".ssh"),
            self.home.join(".gnupg"),
            self.home.join(".aws"),
            self.home.join(".azure"),
            self.home.join(".kube"),
            self.home.join(r".docker\config.json"),
            appdata.join(r"gcloud"),
        ]);
        roots
    }
    fn catalog_source(&self) -> &'static str {
        include_str!("../../rules/windows.toml")
    }
    fn move_to_trash(&self, paths: &[PathBuf]) -> Result<(), trash::Error> {
        trash::delete_all(paths)
    }
    fn recycle_bin_size(&self) -> Option<u64> {
        let items = trash::os_limited::list().ok()?;
        Some(
            items
                .iter()
                .filter_map(|i| trash::os_limited::metadata(i).ok())
                .map(|m| match m.size {
                    trash::TrashItemSize::Bytes(b) => b,
                    trash::TrashItemSize::Entries(_) => 0,
                })
                .sum(),
        )
    }
    fn empty_recycle_bin(&self) -> io::Result<()> {
        let items = trash::os_limited::list().map_err(io::Error::other)?;
        trash::os_limited::purge_all(items).map_err(io::Error::other)
    }
    fn running_processes(&self) -> Vec<String> {
        running_processes_sysinfo()
    }
    fn volume_summary(&self) -> Option<VolumeSummary> {
        volume_for(&self.home, None)
    }
}
