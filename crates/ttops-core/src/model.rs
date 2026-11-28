//! Data types shared across the core and the Tauri boundary (see data-model.md).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Unix time in milliseconds.
pub type Timestamp = u64;

pub fn now_ms() -> Timestamp {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Category {
    /// Safe regenerable caches.
    A,
    /// Developer build outputs.
    B,
    /// Large user data, review only.
    C,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RuleKind {
    /// The directory is one item; cleaning removes its contents and keeps the directory.
    DirContents,
    /// The directory itself is one item.
    Dir,
    /// Every immediate child of the directory is its own item.
    Children,
    /// The OS Trash / Recycle Bin.
    Trash,
    ContainerBuildCache,
    ContainerImages,
    ProjectOutputs,
    LargeFiles,
    /// Identical files under the user's review roots.
    DuplicateFiles,
    /// `brew cleanup` (old versions and downloads), previewed by dry run.
    BrewCleanup,
    /// `brew autoremove` (formulae no longer needed), previewed by dry run.
    BrewAutoremove,
    /// Untagged container images.
    ContainerDanglingImages,
    /// Containers that are not running.
    ContainerStopped,
    /// Unused container volumes (review only).
    ContainerVolumes,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputPattern {
    pub name: String,
    /// When non-empty, the output only counts if its parent contains one of these files.
    #[serde(default)]
    pub markers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    pub id: String,
    pub name: String,
    pub category: Category,
    pub kind: RuleKind,
    #[serde(default)]
    pub paths: Vec<String>,
    #[serde(default)]
    pub regenerable: bool,
    #[serde(default)]
    pub preselect: bool,
    #[serde(default)]
    pub review_only: bool,
    #[serde(default)]
    pub guidance: Option<String>,
    #[serde(default)]
    pub process_names: Vec<String>,
    #[serde(default)]
    pub permanent_only: bool,
    /// Only for `projectOutputs`.
    #[serde(default)]
    pub outputs: Vec<OutputPattern>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum Target {
    Path {
        path: PathBuf,
        /// Remove the directory's contents but keep the directory.
        contents_only: bool,
    },
    ContainerImage {
        id: String,
        repo_tag: String,
    },
    ContainerBuildCache,
    RecycleBin,
    /// An application bundle and its leftovers (App Uninstaller only; always Trash).
    App {
        path: PathBuf,
        bundle_id: String,
        executable: String,
        leftovers: Vec<PathBuf>,
    },
    /// A command run by an external tool; always permanent.
    Tool {
        command: ToolCommand,
    },
    /// A stopped container.
    Container {
        id: String,
        name: String,
    },
    /// Nothing to execute (review-only entries such as container volumes).
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ToolCommand {
    BrewCleanup,
    BrewAutoremove,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanItem {
    pub id: String,
    pub rule_id: String,
    pub category: Category,
    pub target: Target,
    pub label: String,
    pub size_bytes: u64,
    pub last_modified: Option<Timestamp>,
    pub project_path: Option<PathBuf>,
    pub project_age_days: Option<u32>,
    pub selectable: bool,
    pub preselected: bool,
    pub permanent_only: bool,
    pub guidance: Option<String>,
    /// Items sharing a group id are identical files.
    #[serde(default)]
    pub duplicate_group: Option<String>,
    /// Days since the file was last read or written.
    #[serde(default)]
    pub idle_days: Option<u32>,
    /// Extra lines shown in the preview (for example uninstall leftovers or formulae).
    #[serde(default)]
    pub details: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Unscanned {
    pub path: PathBuf,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ScanStatus {
    Running,
    Complete,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanSession {
    pub id: String,
    pub started_at: Timestamp,
    pub finished_at: Option<Timestamp>,
    pub status: ScanStatus,
    pub items: Vec<ScanItem>,
    pub unscanned: Vec<Unscanned>,
    pub container_available: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Action {
    Trash,
    Permanent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewItem {
    pub item_id: String,
    pub rule_id: String,
    pub label: String,
    pub target: Target,
    pub size_bytes: u64,
    pub action: Action,
    #[serde(default)]
    pub details: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RefusalReason {
    Protected,
    NotSelectable,
    Unknown,
    /// Every copy of a duplicate group was requested; one is kept.
    KeepOneCopy,
    /// The application is running or is an Apple system app.
    AppNotRemovable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Refusal {
    pub item_id: String,
    pub reason: RefusalReason,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupPreview {
    pub id: String,
    /// Action requested by the user; `permanentOnly` items are always permanent.
    pub mode: Action,
    pub items: Vec<PreviewItem>,
    pub refused: Vec<Refusal>,
    pub total_bytes: u64,
    pub requires_irreversible_ack: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SkipReason {
    InUse,
    Locked,
    PermissionDenied,
    Protected,
    NoLongerExists,
    CannotMoveToTrash,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Outcome {
    Succeeded,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemResult {
    pub item_id: String,
    pub label: String,
    pub size_bytes: u64,
    pub outcome: Outcome,
    pub reason: Option<SkipReason>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RunStatus {
    Running,
    Completed,
    Interrupted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupRun {
    pub id: String,
    pub preview_id: String,
    pub started_at: Timestamp,
    pub finished_at: Option<Timestamp>,
    pub mode: Action,
    pub status: RunStatus,
    pub items: Vec<PreviewItem>,
    pub results: Vec<ItemResult>,
    pub freed_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupRunSummary {
    pub id: String,
    pub started_at: Timestamp,
    pub finished_at: Option<Timestamp>,
    pub mode: Action,
    pub status: RunStatus,
    pub freed_bytes: u64,
    pub succeeded: usize,
    pub skipped: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    #[serde(default)]
    pub project_roots: Vec<PathBuf>,
    #[serde(default = "default_threshold")]
    pub large_file_threshold_bytes: u64,
    /// Folders searched for duplicate files.
    #[serde(default)]
    pub review_roots: Vec<PathBuf>,
    /// Smallest file considered for duplicate detection.
    #[serde(default = "default_duplicate_min")]
    pub duplicate_min_bytes: u64,
}

fn default_duplicate_min() -> u64 {
    1 << 20
}

fn default_threshold() -> u64 {
    1 << 30
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            project_roots: Vec::new(),
            large_file_threshold_bytes: default_threshold(),
            review_roots: Vec::new(),
            duplicate_min_bytes: default_duplicate_min(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeSummary {
    pub mount_point: PathBuf,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub free_bytes: u64,
}

/// Where a startup item is registered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StartupScope {
    /// The current user's LaunchAgents; the only scope TTops may toggle.
    UserAgent,
    SystemAgent,
    SystemDaemon,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupItem {
    pub label: String,
    pub plist: PathBuf,
    pub program: Option<String>,
    pub scope: StartupScope,
    pub enabled: bool,
    pub editable: bool,
}

/// Metadata read from an application bundle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleInfo {
    pub bundle_id: String,
    pub name: String,
    pub executable: String,
}
