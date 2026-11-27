//! Startup items (launchd). Listing is read-only; toggling is limited to the
//! current user's LaunchAgents, is reversible, and is audited.

use std::collections::HashMap;
use std::process::Command;

use crate::audit::AuditLog;
use crate::model::{StartupItem, StartupScope};
use crate::platform::Platform;

#[derive(Debug, thiserror::Error)]
pub enum StartupError {
    #[error("confirmation required")]
    AckRequired,
    #[error("invalid label")]
    InvalidLabel,
    #[error("only the current user's launch agents can be changed")]
    NotEditable,
    #[error("user id unavailable")]
    NoUser,
    #[error("launchctl: {0}")]
    Launchctl(String),
    #[error("audit: {0}")]
    Audit(#[from] crate::audit::AuditError),
}

pub trait LaunchControl: Send + Sync {
    /// Label → disabled, as recorded by launchd for `gui/<uid>`.
    fn disabled_labels(&self, uid: u32) -> HashMap<String, bool>;
    fn set_enabled(&self, uid: u32, label: &str, enabled: bool) -> Result<(), String>;
}

pub struct Launchctl;
impl LaunchControl for Launchctl {
    fn disabled_labels(&self, uid: u32) -> HashMap<String, bool> {
        Command::new("/bin/launchctl")
            .args(["print-disabled", &format!("gui/{uid}")])
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| parse_print_disabled(&String::from_utf8_lossy(&output.stdout)))
            .unwrap_or_default()
    }
    fn set_enabled(&self, uid: u32, label: &str, enabled: bool) -> Result<(), String> {
        let verb = if enabled { "enable" } else { "disable" };
        let output = Command::new("/bin/launchctl")
            .args([verb, &format!("gui/{uid}/{label}")])
            .output()
            .map_err(|error| error.to_string())?;
        if output.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).trim().to_owned())
        }
    }
}

/// Parses `launchctl print-disabled` lines such as `"label" => disabled`
/// (older systems print `true`/`false`).
pub fn parse_print_disabled(text: &str) -> HashMap<String, bool> {
    text.lines()
        .filter_map(|line| {
            let (label, state) = line.trim().split_once("=>")?;
            let label = label.trim().trim_matches('"');
            let disabled = match state.trim() {
                "disabled" | "true" => true,
                "enabled" | "false" => false,
                _ => return None,
            };
            (!label.is_empty()).then(|| (label.to_owned(), disabled))
        })
        .collect()
}

fn valid_label(label: &str) -> bool {
    !label.is_empty()
        && label.len() <= 255
        && label
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
}

pub fn list_items(platform: &dyn Platform, launch: &dyn LaunchControl) -> Vec<StartupItem> {
    let user_disabled = platform
        .user_id()
        .map(|uid| launch.disabled_labels(uid))
        .unwrap_or_default();
    let mut items = Vec::new();
    for (dir, scope) in platform.launch_dirs() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut plists: Vec<_> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "plist"))
            .collect();
        plists.sort();
        for plist in plists {
            let data = platform.read_plist(&plist).unwrap_or_default();
            let label = data
                .get("Label")
                .and_then(|v| v.as_str())
                .map(str::to_owned)
                .unwrap_or_else(|| {
                    plist
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned()
                });
            let program = data
                .get("Program")
                .and_then(|v| v.as_str())
                .or_else(|| {
                    data.get("ProgramArguments")
                        .and_then(|v| v.get(0))
                        .and_then(|v| v.as_str())
                })
                .map(str::to_owned);
            let plist_disabled = data
                .get("Disabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let disabled = match scope {
                StartupScope::UserAgent => {
                    user_disabled.get(&label).copied().unwrap_or(plist_disabled)
                }
                _ => plist_disabled,
            };
            items.push(StartupItem {
                editable: scope == StartupScope::UserAgent && valid_label(&label),
                label,
                plist,
                program,
                scope,
                enabled: !disabled,
            });
        }
    }
    items
}

pub fn set_enabled(
    platform: &dyn Platform,
    launch: &dyn LaunchControl,
    audit: &AuditLog,
    label: &str,
    enabled: bool,
    acknowledged: bool,
) -> Result<StartupItem, StartupError> {
    if !acknowledged {
        return Err(StartupError::AckRequired);
    }
    if !valid_label(label) {
        return Err(StartupError::InvalidLabel);
    }
    let find = || {
        list_items(platform, launch)
            .into_iter()
            .find(|item| item.label == label)
    };
    let item = find().ok_or(StartupError::NotEditable)?;
    if !item.editable {
        return Err(StartupError::NotEditable);
    }
    let uid = platform.user_id().ok_or(StartupError::NoUser)?;
    launch
        .set_enabled(uid, label, enabled)
        .map_err(StartupError::Launchctl)?;
    audit.startup_toggle(label, enabled)?;
    find().ok_or(StartupError::NotEditable)
}
