//! Local settings persistence. Settings never contain machine-specific defaults.

use std::fs::{File, OpenOptions};
use std::io::{self, BufReader, Write};
use std::path::Path;

use crate::guard::Guard;
use crate::model::Settings;

/// Smallest accepted large-file threshold (1 MiB).
pub const MIN_THRESHOLD_BYTES: u64 = 1 << 20;

/// Validates user-supplied settings: every project root must be an existing,
/// non-protected directory; duplicates are dropped.
pub fn validate(mut settings: Settings, guard: &Guard) -> Result<Settings, String> {
    if settings.large_file_threshold_bytes < MIN_THRESHOLD_BYTES {
        return Err("largeFileThresholdTooSmall".into());
    }
    let mut roots = Vec::new();
    for root in settings.project_roots {
        if !root.is_absolute() || !root.is_dir() {
            return Err(format!("notADirectory: {}", root.display()));
        }
        guard
            .check(&root)
            .map_err(|_| format!("protected: {}", root.display()))?;
        if !roots.contains(&root) {
            roots.push(root);
        }
    }
    settings.project_roots = roots;
    Ok(settings)
}

#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("I/O: {0}")]
    Io(#[from] io::Error),
    #[error("JSON: {0}")]
    Json(#[from] serde_json::Error),
}

pub fn load(path: &Path) -> Result<Settings, SettingsError> {
    let Ok(file) = File::open(path) else {
        return Ok(Settings::default());
    };
    Ok(serde_json::from_reader(BufReader::new(file))?)
}

pub fn save(path: &Path, settings: &Settings) -> Result<(), SettingsError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("json.tmp");
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&temporary)?;
    serde_json::to_writer_pretty(&mut file, settings)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    std::fs::rename(temporary, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_settings_use_one_gib_threshold() {
        let temporary = tempfile::tempdir().unwrap();
        let settings = load(&temporary.path().join("missing.json")).unwrap();
        assert_eq!(settings.large_file_threshold_bytes, 1 << 30);
    }
    #[test]
    fn saved_settings_round_trip() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("settings.json");
        let settings = Settings {
            project_roots: vec![temporary.path().join("project")],
            large_file_threshold_bytes: 42,
        };
        save(&path, &settings).unwrap();
        assert_eq!(load(&path).unwrap(), settings);
    }
}
