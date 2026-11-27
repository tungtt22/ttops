//! Duplicate file detection under user-chosen review roots (read-only).
//! Files are bucketed by size, then by content hash, and equality is confirmed
//! byte for byte before two files are reported as identical.

use std::collections::HashMap;
use std::fs::File;
use std::hash::Hasher;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use walkdir::WalkDir;

use crate::fsutil::last_modified_ms;
use crate::guard::Guard;
use crate::model::{Rule, ScanSession, ScanStatus, Target};

const SKIP_DIRS: &[&str] = &[".git", "node_modules", ".Trash"];
const BUFFER: usize = 64 * 1024;

pub(crate) fn scan_duplicates(
    rule: &Rule,
    platform: &dyn crate::platform::Platform,
    roots: &[PathBuf],
    min_bytes: u64,
    guard: &Guard,
    session: &mut ScanSession,
    cancel: &AtomicBool,
) {
    let mut by_size: HashMap<u64, Vec<PathBuf>> = HashMap::new();
    for root in roots {
        let mut walker = WalkDir::new(root).follow_links(false).into_iter();
        while let Some(entry) = walker.next() {
            if cancel.load(Ordering::Relaxed) {
                session.status = ScanStatus::Cancelled;
                return;
            }
            let Ok(entry) = entry else { continue };
            let name = entry.file_name().to_string_lossy();
            if entry.file_type().is_dir()
                && (SKIP_DIRS.contains(&name.as_ref())
                    || crate::apps::is_privacy_guarded(platform, entry.path()))
            {
                walker.skip_current_dir();
                continue;
            }
            if !entry.file_type().is_file() {
                continue;
            }
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            if metadata.len() < min_bytes.max(1) || guard.check(entry.path()).is_err() {
                continue;
            }
            let paths = by_size.entry(metadata.len()).or_default();
            if !paths.iter().any(|p| p == entry.path()) {
                paths.push(entry.path().to_path_buf());
            }
        }
    }

    let mut sizes: Vec<_> = by_size.into_iter().filter(|(_, p)| p.len() > 1).collect();
    sizes.sort_by_key(|(size, _)| std::cmp::Reverse(*size));
    for (size, paths) in sizes {
        let mut by_hash: HashMap<u64, Vec<PathBuf>> = HashMap::new();
        for path in paths {
            if cancel.load(Ordering::Relaxed) {
                session.status = ScanStatus::Cancelled;
                return;
            }
            if let Ok(hash) = content_hash(&path) {
                by_hash.entry(hash).or_default().push(path);
            }
        }
        for (hash, mut candidates) in by_hash.into_iter().filter(|(_, p)| p.len() > 1) {
            candidates.sort();
            let first = candidates[0].clone();
            let group: Vec<PathBuf> = candidates
                .into_iter()
                .filter(|path| *path == first || same_content(&first, path).unwrap_or(false))
                .collect();
            if group.len() < 2 {
                continue;
            }
            let group_id = format!("dup-{size}-{hash:016x}");
            for path in group {
                let mut found = crate::scanner::item(
                    rule,
                    Target::Path {
                        path: path.clone(),
                        contents_only: false,
                    },
                    path.display().to_string(),
                    size,
                    last_modified_ms(&path).ok().flatten(),
                );
                found.preselected = false;
                found.duplicate_group = Some(group_id.clone());
                session.items.push(found);
            }
        }
    }
}

fn content_hash(path: &Path) -> io::Result<u64> {
    let mut file = File::open(path)?;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    let mut buffer = vec![0; BUFFER];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            return Ok(hasher.finish());
        }
        hasher.write(&buffer[..read]);
    }
}

fn same_content(left: &Path, right: &Path) -> io::Result<bool> {
    let (mut a, mut b) = (File::open(left)?, File::open(right)?);
    let (mut x, mut y) = (vec![0; BUFFER], vec![0; BUFFER]);
    loop {
        let n = read_full(&mut a, &mut x)?;
        let m = read_full(&mut b, &mut y)?;
        if n != m || x[..n] != y[..m] {
            return Ok(false);
        }
        if n == 0 {
            return Ok(true);
        }
    }
}

fn read_full(file: &mut File, buffer: &mut [u8]) -> io::Result<usize> {
    let mut total = 0;
    while total < buffer.len() {
        let read = file.read(&mut buffer[total..])?;
        if read == 0 {
            break;
        }
        total += read;
    }
    Ok(total)
}
