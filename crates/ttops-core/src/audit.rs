//! Append-only local JSONL audit log.

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::model::{
    Action, CleanupRun, CleanupRunSummary, ItemResult, PreviewItem, RunStatus, Timestamp, now_ms,
};

#[derive(Debug, thiserror::Error)]
pub enum AuditError {
    #[error("I/O: {0}")]
    Io(#[from] io::Error),
    #[error("JSON: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum Record {
    RunStarted {
        run_id: String,
        preview_id: String,
        mode: Action,
        at: Timestamp,
        items: Vec<PreviewItem>,
    },
    Item {
        run_id: String,
        result: ItemResult,
        at: Timestamp,
    },
    RunFinished {
        run_id: String,
        freed_bytes: u64,
        at: Timestamp,
    },
}

pub struct AuditLog {
    path: std::path::PathBuf,
}

impl AuditLog {
    pub fn new(path: impl Into<std::path::PathBuf>) -> Self {
        Self { path: path.into() }
    }
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn append(&self, record: &Record) -> Result<(), AuditError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        serde_json::to_writer(&mut file, record)?;
        file.write_all(b"\n")?;
        file.flush()?;
        Ok(())
    }
    pub fn run_started(&self, run: &CleanupRun) -> Result<(), AuditError> {
        self.append(&Record::RunStarted {
            run_id: run.id.clone(),
            preview_id: run.preview_id.clone(),
            mode: run.mode,
            at: run.started_at,
            items: run.items.clone(),
        })
    }
    pub fn item(&self, run_id: &str, result: &ItemResult) -> Result<(), AuditError> {
        self.append(&Record::Item {
            run_id: run_id.to_owned(),
            result: result.clone(),
            at: now_ms(),
        })
    }
    pub fn run_finished(&self, run_id: &str, freed_bytes: u64) -> Result<(), AuditError> {
        self.append(&Record::RunFinished {
            run_id: run_id.to_owned(),
            freed_bytes,
            at: now_ms(),
        })
    }
    pub fn read_runs(&self) -> Result<Vec<CleanupRun>, AuditError> {
        let Ok(file) = File::open(&self.path) else {
            return Ok(Vec::new());
        };
        let mut runs: HashMap<String, CleanupRun> = HashMap::new();
        for line in BufReader::new(file).lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let record: Record = serde_json::from_str(&line)?;
            match record {
                Record::RunStarted {
                    run_id,
                    preview_id,
                    mode,
                    at,
                    items,
                } => {
                    runs.insert(
                        run_id.clone(),
                        CleanupRun {
                            id: run_id,
                            preview_id,
                            started_at: at,
                            finished_at: None,
                            mode,
                            status: RunStatus::Interrupted,
                            items,
                            results: Vec::new(),
                            freed_bytes: 0,
                        },
                    );
                }
                Record::Item { run_id, result, .. } => {
                    if let Some(run) = runs.get_mut(&run_id) {
                        run.results.push(result);
                    }
                }
                Record::RunFinished {
                    run_id,
                    freed_bytes,
                    at,
                } => {
                    if let Some(run) = runs.get_mut(&run_id) {
                        run.finished_at = Some(at);
                        run.freed_bytes = freed_bytes;
                        run.status = RunStatus::Completed;
                    }
                }
            }
        }
        let mut values: Vec<_> = runs.into_values().collect();
        values.sort_by_key(|run| std::cmp::Reverse(run.started_at));
        Ok(values)
    }
    pub fn summaries(&self) -> Result<Vec<CleanupRunSummary>, AuditError> {
        Ok(self
            .read_runs()?
            .into_iter()
            .map(|run| CleanupRunSummary {
                id: run.id,
                started_at: run.started_at,
                finished_at: run.finished_at,
                mode: run.mode,
                status: run.status,
                freed_bytes: run.freed_bytes,
                succeeded: run
                    .results
                    .iter()
                    .filter(|r| matches!(r.outcome, crate::model::Outcome::Succeeded))
                    .count(),
                skipped: run
                    .results
                    .iter()
                    .filter(|r| matches!(r.outcome, crate::model::Outcome::Skipped))
                    .count(),
            })
            .collect())
    }
}
