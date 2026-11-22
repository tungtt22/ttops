use std::sync::{Arc, Mutex};

use ttops_core::{
    audit::AuditLog,
    cleaner::Cleaner,
    container::{ContainerProvider, DockerProvider},
    model::ScanSession,
    platform::Platform,
};

pub struct AppState {
    pub platform: Arc<dyn Platform>,
    pub container: Arc<dyn ContainerProvider>,
    pub cleaner: Mutex<Cleaner>,
    pub scan: Mutex<Option<ScanSession>>,
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
            scan: Mutex::new(None),
        }
    }
}
