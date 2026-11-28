//! Local settings persistence. Settings never contain machine-specific defaults.

use std::fs::{File, OpenOptions};
use std::io::{self, BufReader, Write};
use std::path::Path;

use crate::model::Settings;

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
