//! App Uninstaller scan (read-only): application bundles, their leftovers in the
//! user Library matched by exact bundle id, and orphaned leftovers of removed apps.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::fsutil::{last_modified_ms, path_size_until_cancelled};
use crate::model::{
    BundleInfo, Category, ScanItem, ScanSession, ScanStatus, Target, Unscanned, now_ms,
};
use crate::platform::Platform;

static NEXT_APP_SCAN: AtomicU64 = AtomicU64::new(1);

/// Library sub-folders that hold per-app data.
const LEFTOVER_DIRS: &[&str] = &[
    "Application Support",
    "Caches",
    "Containers",
    "Group Containers",
    "HTTPStorages",
    "Logs",
    "Preferences",
    "Saved Application State",
    "WebKit",
    "LaunchAgents",
];

/// Library folders whose contents macOS privacy protection (App Data) guards: opening
/// them blocks on a consent prompt, so TTops lists them but never reads inside to size them.
const PRIVACY_GUARDED: &[&str] = &["Containers", "Group Containers"];

/// True for paths inside privacy-guarded Library folders of the platform.
pub fn is_privacy_guarded(platform: &dyn Platform, path: &Path) -> bool {
    platform.library_dir().is_some_and(|library| {
        PRIVACY_GUARDED
            .iter()
            .any(|sub| path.starts_with(library.join(sub)))
    })
}

/// Size of a leftover, or `None` when it must not be measured (privacy-guarded)
/// or measurement was cancelled.
fn leftover_size(library: &Path, path: &Path, cancel: &AtomicBool) -> Option<u64> {
    if PRIVACY_GUARDED
        .iter()
        .any(|sub| path.starts_with(library.join(sub)))
    {
        return None;
    }
    path_size_until_cancelled(path, cancel).ok().flatten()
}

/// Folders scanned for orphans: their entries are named by bundle id by convention.
const ORPHAN_DIRS: &[&str] = &[
    "Caches",
    "Containers",
    "Preferences",
    "Saved Application State",
];

pub const APP_RULE: &str = "app-uninstall";
pub const ORPHAN_RULE: &str = "app-orphans";

pub fn scan_apps(platform: &dyn Platform, cancel: &AtomicBool) -> ScanSession {
    let mut session = ScanSession {
        id: format!(
            "apps-{}-{}",
            now_ms(),
            NEXT_APP_SCAN.fetch_add(1, Ordering::Relaxed)
        ),
        started_at: now_ms(),
        finished_at: None,
        status: ScanStatus::Running,
        items: Vec::new(),
        unscanned: Vec::new(),
        container_available: false,
    };
    let library = platform.library_dir();
    let running = platform.running_processes();
    let mut installed: HashSet<String> = HashSet::new();

    for dir in platform.app_dirs() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut entries: Vec<_> = entries.flatten().map(|entry| entry.path()).collect();
        entries.sort();
        for path in entries {
            if cancel.load(Ordering::Relaxed) {
                session.status = ScanStatus::Cancelled;
                break;
            }
            if !is_bundle(&path) {
                // Sub-folders (for example Utilities) only contribute installed ids.
                if path.is_dir() {
                    collect_nested_ids(platform, &path, &mut installed);
                }
                continue;
            }
            let Some(info) = platform.bundle_info(&path) else {
                session.unscanned.push(Unscanned {
                    path: path.clone(),
                    reason: "bundle metadata unreadable".into(),
                });
                continue;
            };
            installed.insert(info.bundle_id.to_ascii_lowercase());
            let Ok(Some(bundle_size)) = path_size_until_cancelled(&path, cancel) else {
                session.status = ScanStatus::Cancelled;
                break;
            };
            let leftovers = library
                .as_deref()
                .map(|library| find_leftovers(library, &info))
                .unwrap_or_default();
            let leftover_bytes: u64 = library
                .as_deref()
                .map(|library| {
                    leftovers
                        .iter()
                        .filter_map(|p| leftover_size(library, p, cancel))
                        .sum()
                })
                .unwrap_or(0);
            session.items.push(app_item(
                &path,
                &info,
                bundle_size + leftover_bytes,
                leftovers,
                &running,
            ));
        }
    }
    if let Some(library) = library.as_deref()
        && !matches!(session.status, ScanStatus::Cancelled)
    {
        find_orphans(library, &installed, &mut session, cancel);
    }
    if matches!(session.status, ScanStatus::Running) {
        session.status = ScanStatus::Complete;
    }
    session.finished_at = Some(now_ms());
    session
}

pub fn is_apple(bundle_id: &str) -> bool {
    bundle_id.to_ascii_lowercase().starts_with("com.apple.")
}

/// True when the executable of a bundle appears among running process names.
pub fn is_running(executable: &str, running: &[String]) -> bool {
    let executable = executable.to_ascii_lowercase();
    !executable.is_empty() && running.contains(&executable)
}

fn is_bundle(path: &Path) -> bool {
    path.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("app"))
        && std::fs::symlink_metadata(path).is_ok_and(|m| m.is_dir())
}

fn collect_nested_ids(platform: &dyn Platform, dir: &Path, installed: &mut HashSet<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for path in entries.flatten().map(|entry| entry.path()) {
        if is_bundle(&path)
            && let Some(info) = platform.bundle_info(&path)
        {
            installed.insert(info.bundle_id.to_ascii_lowercase());
        }
    }
}

fn app_item(
    path: &Path,
    info: &BundleInfo,
    size_bytes: u64,
    leftovers: Vec<PathBuf>,
    running: &[String],
) -> ScanItem {
    let apple = is_apple(&info.bundle_id);
    let guidance = if apple {
        Some("Apple system app: cannot be removed.".to_owned())
    } else if is_running(&info.executable, running) {
        Some("Running: quit the app before uninstalling.".to_owned())
    } else {
        None
    };
    ScanItem {
        id: format!("{APP_RULE}:{}", path.display()),
        rule_id: APP_RULE.into(),
        category: Category::C,
        details: std::iter::once(path.display().to_string())
            .chain(leftovers.iter().map(|p| p.display().to_string()))
            .collect(),
        target: Target::App {
            path: path.to_path_buf(),
            bundle_id: info.bundle_id.clone(),
            executable: info.executable.clone(),
            leftovers,
        },
        label: info.name.clone(),
        size_bytes,
        last_modified: last_modified_ms(path).ok().flatten(),
        project_path: None,
        project_age_days: None,
        selectable: !apple,
        preselected: false,
        permanent_only: false,
        guidance,
        duplicate_group: None,
        idle_days: None,
    }
}

/// Exact-id matches only, so `com.example.foo` never claims `com.example.foobar`.
fn find_leftovers(library: &Path, info: &BundleInfo) -> Vec<PathBuf> {
    let id = info.bundle_id.to_ascii_lowercase();
    let name = info.name.to_ascii_lowercase();
    let mut found = Vec::new();
    for sub in LEFTOVER_DIRS {
        let Ok(entries) = std::fs::read_dir(library.join(sub)) else {
            continue;
        };
        for entry in entries.flatten() {
            let entry_name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            let matches = entry_name == id
                || entry_name == format!("{id}.plist")
                || entry_name == format!("{id}.savedstate")
                || entry_name == format!("{id}.binarycookies")
                || (*sub == "Group Containers" && entry_name.ends_with(&format!(".{id}")))
                || (*sub == "Application Support" && name.len() >= 3 && entry_name == name);
            if matches {
                found.push(entry.path());
            }
        }
    }
    found.sort();
    found
}

fn looks_like_bundle_id(name: &str) -> bool {
    name.split('.').filter(|part| !part.is_empty()).count() >= 3
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
}

fn owner_installed(id: &str, installed: &HashSet<String>) -> bool {
    installed
        .iter()
        .any(|app| id == app || id.starts_with(&format!("{app}.")))
}

fn find_orphans(
    library: &Path,
    installed: &HashSet<String>,
    session: &mut ScanSession,
    cancel: &AtomicBool,
) {
    for sub in ORPHAN_DIRS {
        let Ok(entries) = std::fs::read_dir(library.join(sub)) else {
            continue;
        };
        let mut entries: Vec<_> = entries.flatten().map(|entry| entry.path()).collect();
        entries.sort();
        for path in entries {
            if cancel.load(Ordering::Relaxed) {
                session.status = ScanStatus::Cancelled;
                return;
            }
            let file = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let id = file
                .strip_suffix(".plist")
                .or_else(|| file.strip_suffix(".savedState"))
                .unwrap_or(&file)
                .to_ascii_lowercase();
            if !looks_like_bundle_id(&id) || is_apple(&id) || owner_installed(&id, installed) {
                continue;
            }
            let measured = leftover_size(library, &path, cancel);
            let size = measured.unwrap_or(0);
            let guidance = if measured.is_none() {
                "No installed app uses this id; it may belong to an app installed elsewhere. Size not measured (protected app data)."
            } else {
                "No installed app uses this id; it may belong to an app installed elsewhere."
            };
            session.items.push(ScanItem {
                id: format!("{ORPHAN_RULE}:{}", path.display()),
                rule_id: ORPHAN_RULE.into(),
                category: Category::C,
                target: Target::Path {
                    path: path.clone(),
                    contents_only: false,
                },
                label: format!("{sub}/{file}"),
                size_bytes: size,
                last_modified: last_modified_ms(&path).ok().flatten(),
                project_path: None,
                project_age_days: None,
                selectable: true,
                preselected: false,
                permanent_only: false,
                guidance: Some(guidance.into()),
                duplicate_group: None,
                idle_days: None,
                details: Vec::new(),
            });
        }
    }
}
