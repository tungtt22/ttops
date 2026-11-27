use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

use ttops_core::{
    audit::AuditLog,
    cleaner::Cleaner,
    container::{ContainerProvider, DockerProvider},
    model::{ScanSession, now_ms},
    platform::Platform,
    startup::{LaunchControl, Launchctl},
    tools::{BrewProvider, ToolProvider},
};

/// Completed scans kept for preview (storage and apps can coexist).
const KEPT_SCANS: usize = 4;

pub struct ScanStore {
    active: Mutex<Option<ActiveScan>>,
    latest: Mutex<Vec<ScanSession>>,
    next_id: AtomicU64,
}

struct ActiveScan {
    id: String,
    cancel: Arc<AtomicBool>,
}

impl Default for ScanStore {
    fn default() -> Self {
        Self {
            active: Mutex::new(None),
            latest: Mutex::new(Vec::new()),
            next_id: AtomicU64::new(1),
        }
    }
}

impl ScanStore {
    pub fn begin(&self) -> Result<(String, Arc<AtomicBool>), String> {
        let mut active = self.active.lock().map_err(|error| error.to_string())?;
        if active.is_some() {
            return Err("scanRunning".into());
        }
        let id = format!(
            "scan-{}-{}",
            now_ms(),
            self.next_id.fetch_add(1, Ordering::Relaxed)
        );
        let cancel = Arc::new(AtomicBool::new(false));
        *active = Some(ActiveScan {
            id: id.clone(),
            cancel: cancel.clone(),
        });
        Ok((id, cancel))
    }

    pub fn cancel(&self, scan_id: &str) -> Result<(), String> {
        let active = self.active.lock().map_err(|error| error.to_string())?;
        let scan = active
            .as_ref()
            .filter(|scan| scan.id == scan_id)
            .ok_or_else(|| "unknown scan".to_string())?;
        scan.cancel.store(true, Ordering::Relaxed);
        Ok(())
    }

    pub fn finish(&self, session: ScanSession) -> Result<(), String> {
        let mut active = self.active.lock().map_err(|error| error.to_string())?;
        if active.as_ref().is_some_and(|scan| scan.id == session.id) {
            *active = None;
        }
        let mut latest = self.latest.lock().map_err(|error| error.to_string())?;
        latest.retain(|scan| scan.id != session.id);
        latest.push(session);
        if latest.len() > KEPT_SCANS {
            latest.remove(0);
        }
        Ok(())
    }

    pub fn get(&self, scan_id: &str) -> Result<ScanSession, String> {
        self.latest
            .lock()
            .map_err(|error| error.to_string())?
            .iter()
            .find(|scan| scan.id == scan_id)
            .cloned()
            .ok_or_else(|| "unknown scan".to_string())
    }
}

pub struct AppState {
    pub platform: Arc<dyn Platform>,
    pub container: Arc<dyn ContainerProvider>,
    pub cleaner: Mutex<Cleaner>,
    pub scans: Arc<ScanStore>,
    pub tools: Arc<dyn ToolProvider>,
    pub launch: Arc<dyn LaunchControl>,
    pub audit: AuditLog,
    pub settings_path: std::path::PathBuf,
    pub settings_lock: Mutex<()>,
}

impl AppState {
    pub fn new(platform: Box<dyn Platform>) -> Self {
        let platform: Arc<dyn Platform> = Arc::from(platform);
        let container: Arc<dyn ContainerProvider> = Arc::new(DockerProvider);
        let app_data = platform.data_dir().join("TTops");
        let audit_path = app_data.join("audit.jsonl");
        let tools: Arc<dyn ToolProvider> = Arc::new(BrewProvider::detect());
        Self {
            cleaner: Mutex::new(
                Cleaner::new(
                    platform.clone(),
                    container.clone(),
                    AuditLog::new(&audit_path),
                )
                .with_tools(tools.clone()),
            ),
            tools,
            launch: Arc::new(Launchctl),
            audit: AuditLog::new(audit_path),
            platform,
            container,
            scans: Arc::new(ScanStore::default()),
            settings_path: app_data.join("settings.json"),
            settings_lock: Mutex::new(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ScanStore;
    use ttops_core::model::{ScanSession, ScanStatus};

    fn session(id: String) -> ScanSession {
        ScanSession {
            id,
            started_at: 0,
            finished_at: Some(1),
            status: ScanStatus::Cancelled,
            items: vec![],
            unscanned: vec![],
            container_available: false,
        }
    }

    #[test]
    fn cancellation_targets_the_active_scan_and_retains_its_result() {
        let scans = ScanStore::default();
        let (scan_id, cancel) = scans.begin().unwrap();

        assert_eq!(scans.cancel("other-scan").unwrap_err(), "unknown scan");
        assert!(!cancel.load(std::sync::atomic::Ordering::Relaxed));
        scans.cancel(&scan_id).unwrap();
        assert!(cancel.load(std::sync::atomic::Ordering::Relaxed));

        scans.finish(session(scan_id.clone())).unwrap();
        assert_eq!(scans.get(&scan_id).unwrap().status, ScanStatus::Cancelled);
        assert!(scans.begin().is_ok());
    }
}
