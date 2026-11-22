//! Read-only scan engine. Filesystem traversal never follows symlinks.

use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::container::ContainerProvider;
use crate::fsutil::{last_modified_ms, path_size};
use crate::model::{
    Category, Rule, RuleKind, ScanItem, ScanSession, ScanStatus, Target, Unscanned, now_ms,
};
use crate::platform::Platform;
use crate::rules::expand_paths;

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
    for rule in rules {
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
    match path_size(path) {
        Ok(size) => session.items.push(item(
            rule,
            Target::Path {
                path: path.to_path_buf(),
                contents_only: matches!(rule.kind, RuleKind::DirContents),
            },
            rule.name.clone(),
            size,
            last_modified_ms(path).ok().flatten(),
        )),
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
fn item(
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
    }
}
