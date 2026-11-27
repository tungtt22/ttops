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
use crate::tools::{NoTools, ToolProvider};

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
    tools: Arc<dyn ToolProvider>,
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
            tools: Arc::new(NoTools),
            audit,
            previews: HashMap::new(),
            used: HashSet::new(),
        }
    }
    /// Uses `tools` for external tool cleanups (Homebrew).
    pub fn with_tools(mut self, tools: Arc<dyn ToolProvider>) -> Self {
        self.tools = tools;
        self
    }
    pub fn preview(
        &mut self,
        scan: &ScanSession,
        item_ids: &[String],
        permanent: bool,
    ) -> CleanupPreview {
        let guard = Guard::new(self.platform.as_ref());
        let requested: HashSet<_> = item_ids.iter().collect();
        let keep = keep_one_copy(scan, &requested);
        let running = self.platform.running_processes();
        let app_dirs = self.platform.app_dirs();
        let mut items = Vec::new();
        let mut refused = Vec::new();
        for item in scan
            .items
            .iter()
            .filter(|item| requested.contains(&item.id))
        {
            if keep.contains(&item.id) {
                refused.push(Refusal {
                    item_id: item.id.clone(),
                    reason: RefusalReason::KeepOneCopy,
                });
                continue;
            }
            if !item.selectable || matches!(item.target, Target::None) {
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
            if let Target::App {
                path,
                bundle_id,
                executable,
                ..
            } = &item.target
            {
                if crate::apps::is_apple(bundle_id) || crate::apps::is_running(executable, &running)
                {
                    refused.push(Refusal {
                        item_id: item.id.clone(),
                        reason: RefusalReason::AppNotRemovable,
                    });
                    continue;
                }
                if guard.check_app_bundle(path, &app_dirs).is_err() {
                    refused.push(Refusal {
                        item_id: item.id.clone(),
                        reason: RefusalReason::Protected,
                    });
                    continue;
                }
            }
            // Uninstalls always go to the Trash; external tools are always permanent.
            let action = match &item.target {
                Target::App { .. } => Action::Trash,
                Target::Tool { .. } | Target::Container { .. } => Action::Permanent,
                _ if permanent || item.permanent_only => Action::Permanent,
                _ => Action::Trash,
            };
            items.push(PreviewItem {
                item_id: item.id.clone(),
                rule_id: item.rule_id.clone(),
                label: item.label.clone(),
                target: item.target.clone(),
                size_bytes: item.size_bytes,
                action,
                details: item.details.clone(),
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
        self.run_with_progress(preview_id, acknowledge_irreversible, |_, _, _, _| {})
    }
    pub fn run_with_progress<F>(
        &mut self,
        preview_id: &str,
        acknowledge_irreversible: bool,
        mut progress: F,
    ) -> Result<CleanupRun, CleanerError>
    where
        F: FnMut(&str, usize, usize, &ItemResult),
    {
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
                let done = run.results.len() + 1;
                progress(&run.id, done, run.items.len(), &result);
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
                let current_size = measured_size(self.platform.as_ref(), path, item.size_bytes);
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
            Target::App {
                path,
                bundle_id,
                executable,
                leftovers,
            } => self.uninstall(item, path, bundle_id, executable, leftovers),
            Target::Tool { command } => match self.tools.run(*command) {
                Ok(()) => vec![success(item)],
                Err(error) => vec![fail(SkipReason::Failed, Some(error))],
            },
            Target::Container { id, .. } => match self.container.remove_container(id) {
                Ok(()) => vec![success(item)],
                Err(error) => vec![fail(SkipReason::InUse, Some(error))],
            },
            Target::None => vec![fail(SkipReason::Failed, Some("review only".into()))],
        }
    }

    /// Re-checks the uninstall exception at execution, then moves the bundle and
    /// each guard-approved leftover to the Trash. Never permanent.
    fn uninstall(
        &self,
        item: &PreviewItem,
        path: &Path,
        bundle_id: &str,
        executable: &str,
        leftovers: &[std::path::PathBuf],
    ) -> Vec<ItemResult> {
        let result = |label: String, size_bytes, reason: Option<SkipReason>, message| ItemResult {
            item_id: item.item_id.clone(),
            label,
            size_bytes,
            outcome: if reason.is_none() {
                Outcome::Succeeded
            } else {
                Outcome::Skipped
            },
            reason,
            message,
        };
        if !path.exists() {
            return vec![result(
                item.label.clone(),
                0,
                Some(SkipReason::NoLongerExists),
                None,
            )];
        }
        if crate::apps::is_running(executable, &self.platform.running_processes()) {
            return vec![result(
                item.label.clone(),
                0,
                Some(SkipReason::InUse),
                Some("quit the app first".into()),
            )];
        }
        let guard = Guard::new(self.platform.as_ref());
        if crate::apps::is_apple(bundle_id)
            || guard
                .check_app_bundle(path, &self.platform.app_dirs())
                .is_err()
        {
            return vec![result(
                item.label.clone(),
                0,
                Some(SkipReason::Protected),
                None,
            )];
        }
        let bundle_size = measured_size(self.platform.as_ref(), path, 0);
        let mut results = vec![
            match delete_path(self.platform.as_ref(), path, Action::Trash) {
                Ok(()) => result(path.display().to_string(), bundle_size, None, None),
                Err(error) => {
                    // Without the bundle gone, leave the leftovers in place too.
                    return vec![result(
                        path.display().to_string(),
                        0,
                        Some(map_error(&error, Action::Trash)),
                        Some(error.to_string()),
                    )];
                }
            },
        ];
        for leftover in leftovers {
            let label = leftover.display().to_string();
            if !leftover.exists() {
                results.push(result(label, 0, Some(SkipReason::NoLongerExists), None));
                continue;
            }
            if guard.check(leftover).is_err() {
                results.push(result(label, 0, Some(SkipReason::Protected), None));
                continue;
            }
            let size = measured_size(self.platform.as_ref(), leftover, 0);
            results.push(
                match delete_path(self.platform.as_ref(), leftover, Action::Trash) {
                    Ok(()) => result(label, size, None, None),
                    Err(error) => result(
                        label,
                        0,
                        Some(map_error(&error, Action::Trash)),
                        Some(error.to_string()),
                    ),
                },
            );
        }
        results
    }
}

/// Size for results without reading inside privacy-guarded folders, which would
/// block on a macOS consent prompt; those report the size known at scan time.
fn measured_size(platform: &dyn Platform, path: &Path, known: u64) -> u64 {
    if crate::apps::is_privacy_guarded(platform, path) {
        return known;
    }
    path_size(path).unwrap_or(known)
}

/// When every copy of a duplicate group is requested, refuse the oldest copy so one remains.
fn keep_one_copy(scan: &ScanSession, requested: &HashSet<&String>) -> HashSet<String> {
    let mut groups: HashMap<&str, Vec<&crate::model::ScanItem>> = HashMap::new();
    for item in &scan.items {
        if let Some(group) = &item.duplicate_group {
            groups.entry(group).or_default().push(item);
        }
    }
    groups
        .values()
        .filter(|members| members.iter().all(|item| requested.contains(&item.id)))
        .filter_map(|members| {
            members
                .iter()
                .min_by_key(|item| (item.last_modified.unwrap_or(u64::MAX), item.id.clone()))
                .map(|item| item.id.clone())
        })
        .collect()
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
