//! Caller-owned acquisition budgets, independent of policy and process output limits.

use anyhow::{Result, bail};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Semaphore;

#[derive(Debug, Clone)]
pub struct CaptureOptions {
    pub max_bytes: usize,
    pub max_file_bytes: usize,
    pub jobs: usize,
    pub timeout: Duration,
    pub path_filter: Option<String>,
}

impl Default for CaptureOptions {
    fn default() -> Self {
        Self {
            max_bytes: super::MAX_SNAPSHOT_BYTES,
            max_file_bytes: super::MAX_FILE_BYTES,
            jobs: 4,
            timeout: Duration::from_secs(120),
            path_filter: None,
        }
    }
}

impl CaptureOptions {
    pub fn validate(&self) -> Result<()> {
        if !(1..=1024 * 1024 * 1024).contains(&self.max_bytes)
            || !(1..=crate::domain::snapshot_budget::MAX_FILE_BYTES).contains(&self.max_file_bytes)
            || !(1..=16).contains(&self.jobs)
            || self.timeout.is_zero()
            || self.timeout > Duration::from_secs(3600)
        {
            bail!(
                "Snapshot budgets require 1..=1073741824 total bytes, 1..=8388608 bytes per file, 1..=16 jobs and a timeout of at most 3600 seconds"
            );
        }
        Ok(())
    }
}

#[derive(Clone)]
pub(super) struct Acquisition {
    pub options: CaptureOptions,
    pub permits: Arc<Semaphore>,
    pub deadline: Instant,
}

impl Acquisition {
    pub fn new(options: &CaptureOptions) -> Result<Self> {
        options.validate()?;
        Ok(Self {
            options: options.clone(),
            permits: Arc::new(Semaphore::new(options.jobs)),
            deadline: Instant::now() + options.timeout,
        })
    }
}
