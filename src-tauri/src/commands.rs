use std::sync::atomic::AtomicBool;

use tauri::{Emitter, State};
use ttops_core::{
    model::{CleanupPreview, CleanupRun, ScanSession, VolumeSummary},
    rules::load_platform_catalog,
    scanner::{NoopProgress, scan},
};

use crate::state::AppState;

fn error(message: impl ToString) -> String {
    message.to_string()
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
) -> Result<ScanSession, String> {
    let rules = load_platform_catalog(state.platform.as_ref()).map_err(error)?;
    let session = scan(
        state.platform.as_ref(),
        &rules,
        state.container.as_ref(),
        &AtomicBool::new(false),
        &NoopProgress,
    );
    let id = session.id.clone();
    let status = session.status;
    *state.scan.lock().map_err(error)? = Some(session.clone());
    app.emit(
        "scan://finished",
        serde_json::json!({"scanId": id, "status": status}),
    )
    .map_err(error)?;
    Ok(session)
}

#[tauri::command]
pub fn cancel_scan(_scan_id: String) {}

#[tauri::command]
pub fn get_scan(scan_id: String, state: State<'_, AppState>) -> Result<ScanSession, String> {
    state
        .scan
        .lock()
        .map_err(error)?
        .as_ref()
        .filter(|scan| scan.id == scan_id)
        .cloned()
        .ok_or_else(|| error("unknown scan"))
}

#[tauri::command]
pub fn preview_cleanup(
    scan_id: String,
    item_ids: Vec<String>,
    permanent: bool,
    state: State<'_, AppState>,
) -> Result<CleanupPreview, String> {
    let scan = state
        .scan
        .lock()
        .map_err(error)?
        .as_ref()
        .filter(|scan| scan.id == scan_id)
        .cloned()
        .ok_or_else(|| error("unknown scan"))?;
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
        .run(&preview_id, acknowledge_irreversible)
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
