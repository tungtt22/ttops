//! Read-only scan engine. Filesystem traversal never follows symlinks.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::container::ContainerProvider;
use walkdir::WalkDir;

use crate::fsutil::{last_modified_ms, path_size_until_cancelled};
use crate::guard::Guard;
use crate::model::{
    Category, Rule, RuleKind, ScanItem, ScanSession, ScanStatus, Settings, Target, Unscanned,
    now_ms,
};
use crate::platform::Platform;
use crate::rules::expand_paths;
use crate::tools::{NoTools, ToolProvider};

static NEXT_SCAN: AtomicU64 = AtomicU64::new(1);
pub trait ProgressSink: Send + Sync {
    fn progress(&self, current_rule: &str, items_found: usize, bytes_found: u64);
}
pub struct NoopProgress;
impl ProgressSink for NoopProgress {
    fn progress(&self, _: &str, _: usize, _: u64) {}
}

pub fn scan(
    platform: &dyn Platform,
    rules: &[Rule],
    container: &dyn ContainerProvider,
    cancel: &AtomicBool,
    progress: &dyn ProgressSink,
) -> ScanSession {
    scan_with_id(
        format!(
            "scan-{}-{}",
            now_ms(),
            NEXT_SCAN.fetch_add(1, Ordering::Relaxed)
        ),
        platform,
        rules,
        container,
        cancel,
        progress,
    )
}

pub fn scan_with_id(
    id: String,
    platform: &dyn Platform,
    rules: &[Rule],
    container: &dyn ContainerProvider,
    cancel: &AtomicBool,
    progress: &dyn ProgressSink,
) -> ScanSession {
    scan_with_settings(
        id,
        platform,
        rules,
        &Settings::default(),
        container,
        cancel,
        progress,
    )
}

pub fn scan_with_settings(
    id: String,
    platform: &dyn Platform,
    rules: &[Rule],
    settings: &Settings,
    container: &dyn ContainerProvider,
    cancel: &AtomicBool,
    progress: &dyn ProgressSink,
) -> ScanSession {
    scan_with_tools(
        id, platform, rules, settings, container, &NoTools, cancel, progress,
    )
}

/// Scans every rule. `largeFiles` rules run last so they can skip paths that
/// other rules already claimed and sizes are never counted twice.
#[allow(clippy::too_many_arguments)]
pub fn scan_with_tools(
    id: String,
    platform: &dyn Platform,
    rules: &[Rule],
    settings: &Settings,
    container: &dyn ContainerProvider,
    tools: &dyn ToolProvider,
    cancel: &AtomicBool,
    progress: &dyn ProgressSink,
) -> ScanSession {
    let guard = Guard::new(platform);
    let ordered = rules
        .iter()
        .filter(|rule| rule.kind != RuleKind::LargeFiles)
        .chain(
            rules
                .iter()
                .filter(|rule| rule.kind == RuleKind::LargeFiles),
        );
    let mut session = ScanSession {
        id,
        started_at: now_ms(),
        finished_at: None,
        status: ScanStatus::Running,
        items: Vec::new(),
        unscanned: Vec::new(),
        container_available: container.available(),
    };
    if cancel.load(Ordering::Relaxed) {
        session.status = ScanStatus::Cancelled;
    }
    for rule in ordered {
        if cancel.load(Ordering::Relaxed) {
            session.status = ScanStatus::Cancelled;
            break;
        }
        match rule.kind {
            RuleKind::ContainerBuildCache if session.container_available => {
                if let Ok(size) = container.build_cache_bytes()
                    && size > 0
                {
                    session.items.push(item(
                        rule,
                        Target::ContainerBuildCache,
                        rule.name.clone(),
                        size,
                        None,
                    ));
                }
            }
            RuleKind::ContainerImages if session.container_available => {
                if let Ok(images) = container.unused_images() {
                    for image in images {
                        session.items.push(item(
                            rule,
                            Target::ContainerImage {
                                id: image.id,
                                repo_tag: image.repo_tag.clone(),
                            },
                            image.repo_tag,
                            image.size_bytes,
                            None,
                        ));
                    }
                }
            }
            RuleKind::Trash => {
                for path in expand_paths(rule, platform) {
                    scan_children(rule, &path, &mut session, cancel);
                }
            }
            RuleKind::Dir | RuleKind::DirContents | RuleKind::Children => {
                for path in expand_paths(rule, platform) {
                    scan_path(rule, &path, &mut session, cancel);
                }
            }
            RuleKind::ProjectOutputs => {
                for root in &settings.project_roots {
                    scan_project_outputs(rule, root, &guard, &mut session, cancel);
                }
            }
            RuleKind::DuplicateFiles => {
                crate::duplicates::scan_duplicates(
                    rule,
                    platform,
                    &settings.review_roots,
                    settings.duplicate_min_bytes,
                    &guard,
                    &mut session,
                    cancel,
                );
            }
            RuleKind::BrewCleanup if tools.brew_available() => {
                if let Ok((bytes, lines)) = tools.brew_cleanup_preview()
                    && (bytes > 0 || !lines.is_empty())
                {
                    let mut found = external(
                        rule,
                        Target::Tool {
                            command: crate::model::ToolCommand::BrewCleanup,
                        },
                        rule.name.clone(),
                        bytes,
                    );
                    found.details = lines;
                    session.items.push(found);
                }
            }
            RuleKind::BrewAutoremove if tools.brew_available() => {
                if let Ok(formulae) = tools.brew_autoremove_preview()
                    && !formulae.is_empty()
                {
                    let mut found = external(
                        rule,
                        Target::Tool {
                            command: crate::model::ToolCommand::BrewAutoremove,
                        },
                        format!("{} ({})", rule.name, formulae.len()),
                        0,
                    );
                    found.details = formulae;
                    session.items.push(found);
                }
            }
            RuleKind::ContainerDanglingImages if session.container_available => {
                for image in container.dangling_images().unwrap_or_default() {
                    session.items.push(external(
                        rule,
                        Target::ContainerImage {
                            id: image.id.clone(),
                            repo_tag: image.repo_tag,
                        },
                        format!("Untagged image {}", image.id),
                        image.size_bytes,
                    ));
                }
            }
            RuleKind::ContainerStopped if session.container_available => {
                for stopped in container.stopped_containers().unwrap_or_default() {
                    session.items.push(external(
                        rule,
                        Target::Container {
                            id: stopped.id,
                            name: stopped.name.clone(),
                        },
                        format!("Stopped container {}", stopped.name),
                        stopped.size_bytes,
                    ));
                }
            }
            RuleKind::ContainerVolumes if session.container_available => {
                for volume in container.unused_volumes().unwrap_or_default() {
                    let mut found = external(rule, Target::None, volume.clone(), 0);
                    found.id = format!("{}:{volume}", rule.id);
                    found.selectable = false;
                    session.items.push(found);
                }
            }
            RuleKind::LargeFiles => {
                for root in expand_paths(rule, platform) {
                    scan_large_files(
                        rule,
                        platform,
                        &root,
                        settings.large_file_threshold_bytes,
                        &guard,
                        &mut session,
                        cancel,
                    );
                }
            }
            _ => {}
        }
        let bytes = session.items.iter().map(|item| item.size_bytes).sum();
        progress.progress(&rule.id, session.items.len(), bytes);
    }
    if matches!(session.status, ScanStatus::Running) {
        session.status = ScanStatus::Complete;
    }
    session.finished_at = Some(now_ms());
    session
}
fn scan_path(rule: &Rule, path: &Path, session: &mut ScanSession, cancel: &AtomicBool) {
    if !path.exists() {
        return;
    }
    if matches!(rule.kind, RuleKind::Children) {
        scan_children(rule, path, session, cancel);
        return;
    }
    if cancel.load(Ordering::Relaxed) {
        return;
    }
    match path_size_until_cancelled(path, cancel) {
        Ok(Some(size)) => session.items.push(item(
            rule,
            Target::Path {
                path: path.to_path_buf(),
                contents_only: matches!(rule.kind, RuleKind::DirContents),
            },
            rule.name.clone(),
            size,
            last_modified_ms(path).ok().flatten(),
        )),
        Ok(None) => session.status = ScanStatus::Cancelled,
        Err(error) => session.unscanned.push(Unscanned {
            path: path.to_path_buf(),
            reason: error.kind().to_string(),
        }),
    }
}
fn scan_children(rule: &Rule, directory: &Path, session: &mut ScanSession, cancel: &AtomicBool) {
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) => {
            session.unscanned.push(Unscanned {
                path: directory.to_path_buf(),
                reason: error.kind().to_string(),
            });
            return;
        }
    };
    for entry in entries.flatten() {
        if cancel.load(Ordering::Relaxed) {
            return;
        }
        scan_path(
            &Rule {
                kind: RuleKind::Dir,
                ..rule.clone()
            },
            &entry.path(),
            session,
            cancel,
        );
    }
}
const MS_PER_DAY: u64 = 86_400_000;

/// Finds build outputs under a user-chosen root. Matching folders are not
/// descended into, so only the outermost output is listed.
fn scan_project_outputs(
    rule: &Rule,
    root: &Path,
    guard: &Guard,
    session: &mut ScanSession,
    cancel: &AtomicBool,
) {
    let mut walker = WalkDir::new(root).follow_links(false).into_iter();
    while let Some(entry) = walker.next() {
        if cancel.load(Ordering::Relaxed) {
            session.status = ScanStatus::Cancelled;
            return;
        }
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                if error.depth() == 0 {
                    session.unscanned.push(Unscanned {
                        path: root.to_path_buf(),
                        reason: error.to_string(),
                    });
                }
                continue;
            }
        };
        if entry.depth() == 0 || !entry.file_type().is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy();
        if name == ".git" {
            walker.skip_current_dir();
            continue;
        }
        let path = entry.path();
        let Some(project) = path.parent() else {
            continue;
        };
        let matched = rule.outputs.iter().any(|output| {
            output.name == name
                && (output.markers.is_empty()
                    || output
                        .markers
                        .iter()
                        .any(|marker| project.join(marker).exists()))
        });
        if !matched {
            continue;
        }
        walker.skip_current_dir();
        if guard.check(path).is_err() {
            continue;
        }
        match path_size_until_cancelled(path, cancel) {
            Ok(Some(size)) => {
                let mut found = item(
                    rule,
                    Target::Path {
                        path: path.to_path_buf(),
                        contents_only: false,
                    },
                    format!("{name} — {}", project.display()),
                    size,
                    last_modified_ms(path).ok().flatten(),
                );
                found.project_path = Some(project.to_path_buf());
                found.project_age_days = project_age_days(rule, project);
                session.items.push(found);
            }
            Ok(None) => {
                session.status = ScanStatus::Cancelled;
                return;
            }
            Err(error) => session.unscanned.push(Unscanned {
                path: path.to_path_buf(),
                reason: error.kind().to_string(),
            }),
        }
    }
}

/// Days since the newest top-level project entry changed, ignoring outputs.
fn project_age_days(rule: &Rule, project: &Path) -> Option<u32> {
    let newest = std::fs::read_dir(project)
        .ok()?
        .flatten()
        .filter(|entry| {
            let name = entry.file_name();
            !rule.outputs.iter().any(|output| *output.name == *name)
        })
        .filter_map(|entry| last_modified_ms(&entry.path()).ok().flatten())
        .max()
        .or_else(|| last_modified_ms(project).ok().flatten())?;
    Some((now_ms().saturating_sub(newest) / MS_PER_DAY) as u32)
}

/// Lists files at or above the threshold that no earlier rule claimed and
/// the guard does not protect. Unreadable folders are skipped silently.
fn scan_large_files(
    rule: &Rule,
    platform: &dyn Platform,
    root: &Path,
    threshold: u64,
    guard: &Guard,
    session: &mut ScanSession,
    cancel: &AtomicBool,
) {
    let claimed: Vec<PathBuf> = session
        .items
        .iter()
        .filter_map(|item| match &item.target {
            Target::Path { path, .. } => Some(path.clone()),
            _ => None,
        })
        .collect();
    let completed = walk_unprotected(platform, root, guard, &claimed, cancel, |entry| {
        let path = entry.path();
        if !entry.file_type().is_file() {
            return;
        }
        let Ok(metadata) = entry.metadata() else {
            return;
        };
        if metadata.len() < threshold || guard.check(path).is_err() {
            return;
        }
        let mut found = item(
            rule,
            Target::Path {
                path: path.to_path_buf(),
                contents_only: false,
            },
            path.display().to_string(),
            metadata.len(),
            last_modified_ms(path).ok().flatten(),
        );
        found.idle_days = idle_days(&metadata);
        session.items.push(found);
    });
    if !completed {
        session.status = ScanStatus::Cancelled;
    }
}

/// Walks `root` without following links and calls `visit` for every entry,
/// never descending into claimed paths or guard-protected subtrees (for example
/// `~/Library` or a credentials folder) and never opening other apps'
/// privacy-guarded containers, which block on a consent prompt. Returns false
/// when cancelled.
fn walk_unprotected(
    platform: &dyn Platform,
    root: &Path,
    guard: &Guard,
    claimed: &[PathBuf],
    cancel: &AtomicBool,
    mut visit: impl FnMut(&walkdir::DirEntry),
) -> bool {
    let mut walker = WalkDir::new(root).follow_links(false).into_iter();
    while let Some(entry) = walker.next() {
        if cancel.load(Ordering::Relaxed) {
            return false;
        }
        let Ok(entry) = entry else {
            continue;
        };
        let path = entry.path();
        let skip = claimed.iter().any(|claim| path.starts_with(claim))
            || (entry.file_type().is_dir() && crate::apps::is_privacy_guarded(platform, path))
            || (entry.depth() > 0 && entry.file_type().is_dir() && guard.protects_subtree(path));
        if skip {
            if entry.file_type().is_dir() {
                walker.skip_current_dir();
            }
            continue;
        }
        visit(&entry);
    }
    true
}

/// Days since the newest of access and modification time.
pub(crate) fn idle_days(metadata: &std::fs::Metadata) -> Option<u32> {
    let last = [metadata.accessed().ok(), metadata.modified().ok()]
        .into_iter()
        .flatten()
        .max()?;
    let elapsed = std::time::SystemTime::now()
        .duration_since(last)
        .unwrap_or_default();
    Some((elapsed.as_secs() / 86_400) as u32)
}

/// Items executed by an external CLI: never preselected, always permanent.
fn external(rule: &Rule, target: Target, label: String, size_bytes: u64) -> ScanItem {
    let mut found = item(rule, target, label, size_bytes, None);
    found.preselected = false;
    found.permanent_only = true;
    found
}

pub(crate) fn item(
    rule: &Rule,
    target: Target,
    label: String,
    size_bytes: u64,
    last_modified: Option<u64>,
) -> ScanItem {
    let id = format!(
        "{}:{}",
        rule.id,
        match &target {
            Target::Path { path, .. } => path.to_string_lossy().to_string(),
            Target::ContainerImage { id, .. } => id.clone(),
            Target::ContainerBuildCache => "build-cache".into(),
            Target::RecycleBin => "recycle-bin".into(),
            Target::App { path, .. } => path.to_string_lossy().to_string(),
            Target::Tool { command } => format!("{command:?}"),
            Target::Container { id, .. } => id.clone(),
            Target::None => "none".into(),
        }
    );
    ScanItem {
        id,
        rule_id: rule.id.clone(),
        category: rule.category,
        target,
        label,
        size_bytes,
        last_modified,
        project_path: None,
        project_age_days: None,
        selectable: !rule.review_only,
        preselected: rule.category == Category::A && rule.preselect && !rule.review_only,
        permanent_only: rule.permanent_only,
        guidance: rule.guidance.clone(),
        duplicate_group: None,
        idle_days: None,
        details: Vec::new(),
    }
}

#[cfg(test)]
mod walk_tests {
    use super::*;
    use crate::platform::{Platform, TestPlatform};

    #[test]
    fn walk_never_enters_protected_or_claimed_subtrees() {
        let temp = tempfile::tempdir().unwrap();
        let platform = TestPlatform::new(temp.path(), "");
        let home = platform.home();
        let ssh = home.join(".ssh");
        for dir in [
            ssh.join("keys/deep"),
            home.join("keys/credentials/inner"),
            home.join("claimed/inner"),
            home.join("videos"),
        ] {
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("file.bin"), b"x").unwrap();
        }
        let guard = Guard::new(&platform);
        assert!(
            guard.protects_subtree(&ssh),
            "test fixture must protect ~/.ssh"
        );

        let mut visited = Vec::new();
        let done = walk_unprotected(
            &platform,
            &home,
            &guard,
            &[home.join("claimed")],
            &AtomicBool::new(false),
            |entry| visited.push(entry.path().to_path_buf()),
        );

        assert!(done);
        assert!(visited.contains(&home.join("videos/file.bin")));
        assert!(visited.contains(&home.join("keys")));
        for blocked in [ssh, home.join("keys/credentials"), home.join("claimed")] {
            assert!(
                visited.iter().all(|path| !path.starts_with(&blocked)),
                "walk entered {blocked:?}: {visited:?}"
            );
        }
    }
}
