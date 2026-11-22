use std::path::{Path, PathBuf};

use crate::platform::Platform;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("protected path: {0}")]
pub struct Protected(pub PathBuf);

pub struct Guard;

impl Guard {
    pub fn new(_platform: &dyn Platform) -> Self {
        Guard
    }
    pub fn check(&self, _path: &Path) -> Result<(), Protected> {
        Ok(())
    }
}
