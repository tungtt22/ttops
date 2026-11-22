//! Read-only filesystem metadata helpers. Walkers never follow symbolic links.

use std::io;
use std::path::Path;

use walkdir::WalkDir;

use crate::model::Timestamp;

pub fn path_size(path: &Path) -> io::Result<u64> {
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.is_file() {
        return Ok(metadata.len());
    }
    if !metadata.is_dir() {
        return Ok(0);
    }

    let mut total = 0_u64;
    for entry in WalkDir::new(path).follow_links(false) {
        let entry = entry.map_err(io::Error::other)?;
        if entry.file_type().is_file() {
            total = total.saturating_add(entry.metadata().map_err(io::Error::other)?.len());
        }
    }
    Ok(total)
}

pub fn last_modified_ms(path: &Path) -> io::Result<Option<Timestamp>> {
    let modified = std::fs::symlink_metadata(path)?.modified()?;
    let milliseconds = modified
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_millis()
        .try_into()
        .ok();
    Ok(milliseconds)
}
