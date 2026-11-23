use std::{
    sync::Mutex,
    thread,
    time::{Duration, Instant},
};

use serde::Serialize;
use tauri::{Emitter, State};
use ttops_core::{
    model::{CleanupPreview, CleanupRun, ScanSession, VolumeSummary},
    rules::load_platform_catalog,
    scanner::{ProgressSink, scan_with_id},
};

use crate::state::AppState;

fn error(message: impl ToString) -> String {
    message.to_string()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanStarted {
    scan_id: String,
}

struct TauriProgress {
    app: tauri::AppHandle,
    scan_id: String,
    last_sent: Mutex<Option<Instant>>,
}

impl ProgressSink for TauriProgress {
    fn progress(&self, current_rule: &str, items_found: usize, bytes_found: u64) {
        let mut last_sent = match self.last_sent.lock() {
            Ok(last_sent) => last_sent,
            Err(_) => return,
        };
        if last_sent.is_some_and(|last| last.elapsed() < Duration::from_millis(100)) {
            return;
        }
        *last_sent = Some(Instant::now());
        let _ = self.app.emit(
            "scan://progress",
            serde_json::json!({
                "scanId": self.scan_id,
                "currentRule": current_rule,
                "itemsFound": items_found,
                "bytesFound": bytes_found,
            }),
        );
    }
}

#[tauri::command]
pub fn get_volume_summary(state: State<'_, AppState>) -> Result<VolumeSummary, String> {
    state
        .platform
        .volume_summary()
        .ok_or_else(|| error("volume summary unavailable"))
}

#[tauri::command]
pub fn start_scan(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<ScanStarted, String> {
    let rules = load_platform_catalog(state.platform.as_ref()).map_err(error)?;
    let (scan_id, cancel) = state.scans.begin()?;
    let platform = state.platform.clone();
    let container = state.container.clone();
    let scans = state.scans.clone();
    let app_for_scan = app.clone();
    let scan_id_for_thread = scan_id.clone();

    thread::spawn(move || {
        let progress = TauriProgress {
            app: app_for_scan.clone(),
            scan_id: scan_id_for_thread.clone(),
            last_sent: Mutex::new(None),
        };
        let session = scan_with_id(
            scan_id_for_thread.clone(),
            platform.as_ref(),
            &rules,
            container.as_ref(),
            cancel.as_ref(),
            &progress,
        );
        let status = session.status;
        let _ = scans.finish(session);
        let _ = app_for_scan.emit(
            "scan://finished",
            serde_json::json!({"scanId": scan_id_for_thread, "status": status}),
        );
    });

    Ok(ScanStarted { scan_id })
}

#[tauri::command]
pub fn cancel_scan(scan_id: String, state: State<'_, AppState>) -> Result<(), String> {
    state.scans.cancel(&scan_id)
}

#[tauri::command]
pub fn get_scan(scan_id: String, state: State<'_, AppState>) -> Result<ScanSession, String> {
    state.scans.get(&scan_id)
}

#[tauri::command]
pub fn preview_cleanup(
    scan_id: String,
    item_ids: Vec<String>,
    permanent: bool,
    state: State<'_, AppState>,
) -> Result<CleanupPreview, String> {
    let scan = state.scans.get(&scan_id)?;
    Ok(state
        .cleaner
        .lock()
        .map_err(error)?
        .preview(&scan, &item_ids, permanent))
}

#[tauri::command]
pub fn run_cleanup(
    app: tauri::AppHandle,
    preview_id: String,
    acknowledge_irreversible: bool,
    state: State<'_, AppState>,
) -> Result<CleanupRun, String> {
    let run = state
        .cleaner
        .lock()
        .map_err(error)?
        .run_with_progress(&preview_id, acknowledge_irreversible, |run_id, done, total, current| {
            let _ = app.emit(
                "cleanup://progress",
                serde_json::json!({"runId": run_id, "done": done, "total": total, "current": current}),
            );
        })
        .map_err(error)?;
    let succeeded = run
        .results
        .iter()
        .filter(|result| matches!(result.outcome, ttops_core::model::Outcome::Succeeded))
        .count();
    let skipped = run.results.len().saturating_sub(succeeded);
    app.emit("cleanup://finished", serde_json::json!({"runId": run.id, "freedBytes": run.freed_bytes, "succeeded": succeeded, "skipped": skipped})).map_err(error)?;
    Ok(run)
}
