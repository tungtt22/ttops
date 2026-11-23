//! Read-only filesystem metadata helpers. Walkers never follow symbolic links.

use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use walkdir::WalkDir;

use crate::model::Timestamp;

pub fn path_size(path: &Path) -> io::Result<u64> {
    path_size_until_cancelled(path, &AtomicBool::new(false)).map(|size| size.unwrap_or(0))
}

/// Returns `Ok(None)` when cancellation is observed between directory entries.
pub fn path_size_until_cancelled(path: &Path, cancel: &AtomicBool) -> io::Result<Option<u64>> {
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.is_file() {
        return Ok((!cancel.load(Ordering::Relaxed)).then_some(metadata.len()));
    }
    if !metadata.is_dir() {
        return Ok(Some(0));
    }

    let mut total = 0_u64;
    for entry in WalkDir::new(path).follow_links(false) {
        if cancel.load(Ordering::Relaxed) {
            return Ok(None);
        }
        let entry = entry.map_err(io::Error::other)?;
        if entry.file_type().is_file() {
            total = total.saturating_add(entry.metadata().map_err(io::Error::other)?.len());
        }
    }
    Ok(Some(total))
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
