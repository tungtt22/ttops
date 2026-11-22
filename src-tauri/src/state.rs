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
};

pub struct ScanStore {
    active: Mutex<Option<ActiveScan>>,
    latest: Mutex<Option<ScanSession>>,
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
            latest: Mutex::new(None),
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
        *self.latest.lock().map_err(|error| error.to_string())? = Some(session);
        Ok(())
    }

    pub fn get(&self, scan_id: &str) -> Result<ScanSession, String> {
        self.latest
            .lock()
            .map_err(|error| error.to_string())?
            .as_ref()
            .filter(|scan| scan.id == scan_id)
            .cloned()
            .ok_or_else(|| "unknown scan".to_string())
    }
}

pub struct AppState {
    pub platform: Arc<dyn Platform>,
    pub container: Arc<dyn ContainerProvider>,
    pub cleaner: Mutex<Cleaner>,
    pub scans: Arc<ScanStore>,
}

impl AppState {
    pub fn new(platform: Box<dyn Platform>) -> Self {
        let platform: Arc<dyn Platform> = Arc::from(platform);
        let container: Arc<dyn ContainerProvider> = Arc::new(DockerProvider);
        let audit = AuditLog::new(platform.data_dir().join("TTops").join("audit.jsonl"));
        Self {
            cleaner: Mutex::new(Cleaner::new(platform.clone(), container.clone(), audit)),
            platform,
            container,
            scans: Arc::new(ScanStore::default()),
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
