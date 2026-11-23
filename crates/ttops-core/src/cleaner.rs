//! Guarded, single-use cleanup previews and execution.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::audit::{AuditError, AuditLog};
use crate::container::ContainerProvider;
use crate::fsutil::path_size;
use crate::guard::Guard;
use crate::model::{
    Action, CleanupPreview, CleanupRun, ItemResult, Outcome, PreviewItem, Refusal, RefusalReason,
    RunStatus, ScanSession, SkipReason, Target, now_ms,
};
use crate::platform::Platform;

static NEXT_PREVIEW: AtomicU64 = AtomicU64::new(1);
#[derive(Debug, thiserror::Error)]
pub enum CleanerError {
    #[error("unknown preview")]
    UnknownPreview,
    #[error("preview already used")]
    PreviewUsed,
    #[error("irreversible acknowledgement required")]
    AckRequired,
    #[error("audit: {0}")]
    Audit(#[from] AuditError),
}

pub struct Cleaner {
    platform: Arc<dyn Platform>,
    container: Arc<dyn ContainerProvider>,
    audit: AuditLog,
    previews: HashMap<String, CleanupPreview>,
    used: HashSet<String>,
}
impl Cleaner {
    pub fn new(
        platform: Arc<dyn Platform>,
        container: Arc<dyn ContainerProvider>,
        audit: AuditLog,
    ) -> Self {
        Self {
            platform,
            container,
            audit,
            previews: HashMap::new(),
            used: HashSet::new(),
        }
    }
    pub fn preview(
        &mut self,
        scan: &ScanSession,
        item_ids: &[String],
        permanent: bool,
    ) -> CleanupPreview {
        let guard = Guard::new(self.platform.as_ref());
        let requested: HashSet<_> = item_ids.iter().collect();
        let mut items = Vec::new();
        let mut refused = Vec::new();
        for item in scan
            .items
            .iter()
            .filter(|item| requested.contains(&item.id))
        {
            if !item.selectable {
                refused.push(Refusal {
                    item_id: item.id.clone(),
                    reason: RefusalReason::NotSelectable,
                });
                continue;
            }
            if let Target::Path { path, .. } = &item.target
                && guard.check(path).is_err()
            {
                refused.push(Refusal {
                    item_id: item.id.clone(),
                    reason: RefusalReason::Protected,
                });
                continue;
            }
            let action = if permanent || item.permanent_only {
                Action::Permanent
            } else {
                Action::Trash
            };
            items.push(PreviewItem {
                item_id: item.id.clone(),
                rule_id: item.rule_id.clone(),
                label: item.label.clone(),
                target: item.target.clone(),
                size_bytes: item.size_bytes,
                action,
            });
        }
        let mode = if items.iter().any(|item| item.action == Action::Permanent) {
            Action::Permanent
        } else {
            Action::Trash
        };
        let preview = CleanupPreview {
            id: format!(
                "preview-{}-{}",
                now_ms(),
                NEXT_PREVIEW.fetch_add(1, Ordering::Relaxed)
            ),
            mode,
            total_bytes: items.iter().map(|item| item.size_bytes).sum(),
            requires_irreversible_ack: mode == Action::Permanent,
            items,
            refused,
        };
        self.previews.insert(preview.id.clone(), preview.clone());
        preview
    }
    pub fn run(
        &mut self,
        preview_id: &str,
        acknowledge_irreversible: bool,
    ) -> Result<CleanupRun, CleanerError> {
        if self.used.contains(preview_id) {
            return Err(CleanerError::PreviewUsed);
        }
        let preview = self
            .previews
            .get(preview_id)
            .cloned()
            .ok_or(CleanerError::UnknownPreview)?;
        if preview.requires_irreversible_ack && !acknowledge_irreversible {
            return Err(CleanerError::AckRequired);
        }
        self.used.insert(preview_id.to_owned());
        let mut run = CleanupRun {
            id: format!("run-{}", now_ms()),
            preview_id: preview.id,
            started_at: now_ms(),
            finished_at: None,
            mode: preview.mode,
            status: RunStatus::Running,
            items: preview.items,
            results: Vec::new(),
            freed_bytes: 0,
        };
        self.audit.run_started(&run)?;
        for item in &run.items {
            for result in self.execute(item) {
                if matches!(result.outcome, Outcome::Succeeded) {
                    run.freed_bytes += result.size_bytes;
                }
                self.audit.item(&run.id, &result)?;
                run.results.push(result);
            }
        }
        run.finished_at = Some(now_ms());
        run.status = RunStatus::Completed;
        self.audit.run_finished(&run.id, run.freed_bytes)?;
        Ok(run)
    }
    fn execute(&self, item: &PreviewItem) -> Vec<ItemResult> {
        let fail = |reason, message| ItemResult {
            item_id: item.item_id.clone(),
            label: item.label.clone(),
            size_bytes: item.size_bytes,
            outcome: Outcome::Skipped,
            reason: Some(reason),
            message,
        };
        match &item.target {
            Target::Path {
                path,
                contents_only,
            } => {
                if !path.exists() {
                    return vec![fail(SkipReason::NoLongerExists, None)];
                }
                if Guard::new(self.platform.as_ref()).check(path).is_err() {
                    return vec![fail(SkipReason::Protected, None)];
                }
                if *contents_only {
                    return delete_contents(self.platform.as_ref(), path, item);
                }
                let current_size = path_size(path).unwrap_or(item.size_bytes);
                let operation = delete_path(self.platform.as_ref(), path, item.action);
                match operation {
                    Ok(()) => vec![ItemResult {
                        item_id: item.item_id.clone(),
                        label: item.label.clone(),
                        size_bytes: current_size,
                        outcome: Outcome::Succeeded,
                        reason: None,
                        message: None,
                    }],
                    Err(error) => vec![fail(
                        map_error(&error, item.action),
                        Some(error.to_string()),
                    )],
                }
            }
            Target::ContainerBuildCache => match self.container.prune_build_cache() {
                Ok(()) => vec![success(item)],
                Err(error) => vec![fail(SkipReason::Failed, Some(error))],
            },
            Target::ContainerImage { id, .. } => match self.container.remove_image(id) {
                Ok(()) => vec![success(item)],
                Err(error) => vec![fail(SkipReason::Failed, Some(error))],
            },
            Target::RecycleBin => vec![fail(
                SkipReason::Failed,
                Some("Recycle Bin cleanup is not available on this platform".into()),
            )],
        }
    }
}
fn success(item: &PreviewItem) -> ItemResult {
    ItemResult {
        item_id: item.item_id.clone(),
        label: item.label.clone(),
        size_bytes: item.size_bytes,
        outcome: Outcome::Succeeded,
        reason: None,
        message: None,
    }
}
fn delete_path(platform: &dyn Platform, path: &Path, action: Action) -> std::io::Result<()> {
    match action {
        Action::Permanent => {
            if path.is_dir() {
                std::fs::remove_dir_all(path)
            } else {
                std::fs::remove_file(path)
            }
        }
        Action::Trash => platform
            .move_to_trash(&[path.to_path_buf()])
            .map_err(std::io::Error::other),
    }
}
fn delete_contents(
    platform: &dyn Platform,
    directory: &Path,
    item: &PreviewItem,
) -> Vec<ItemResult> {
    let fail = |path: &Path, reason, message| ItemResult {
        item_id: item.item_id.clone(),
        label: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        size_bytes: path_size(path).unwrap_or(0),
        outcome: Outcome::Skipped,
        reason: Some(reason),
        message,
    };
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) => {
            return vec![fail(
                directory,
                map_error(&error, item.action),
                Some(error.to_string()),
            )];
        }
    };
    let guard = Guard::new(platform);
    let mut results = Vec::new();
    for entry in entries {
        let path = match entry {
            Ok(entry) => entry.path(),
            Err(error) => {
                results.push(fail(
                    directory,
                    map_error(&error, item.action),
                    Some(error.to_string()),
                ));
                continue;
            }
        };
        // Re-check each child at the last responsible moment; a parent cache is not a safety grant.
        if guard.check(&path).is_err() {
            results.push(fail(&path, SkipReason::Protected, None));
            continue;
        }
        let size_bytes = path_size(&path).unwrap_or(0);
        match delete_path(platform, &path, item.action) {
            Ok(()) => results.push(ItemResult {
                item_id: item.item_id.clone(),
                label: path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
                size_bytes,
                outcome: Outcome::Succeeded,
                reason: None,
                message: None,
            }),
            Err(error) => results.push(fail(
                &path,
                map_error(&error, item.action),
                Some(error.to_string()),
            )),
        }
    }
    results
}
fn map_error(error: &std::io::Error, action: Action) -> SkipReason {
    match error.kind() {
        std::io::ErrorKind::NotFound => SkipReason::NoLongerExists,
        std::io::ErrorKind::PermissionDenied => SkipReason::PermissionDenied,
        _ if action == Action::Trash => SkipReason::CannotMoveToTrash,
        _ => SkipReason::Failed,
    }
}
