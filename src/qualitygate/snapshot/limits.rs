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
    pub scope: crate::domain::check_scope::CheckScope,
    pub exclude: Vec<String>,
    pub unstructured_documents: crate::domain::resource_exclusions::Policy,
    pub protected_paths: Vec<String>,
    /// Internal bounded policy bootstrap; never used as a successful check input.
    pub include: Option<Vec<String>>,
}

impl Default for CaptureOptions {
    fn default() -> Self {
        Self {
            max_bytes: super::MAX_SNAPSHOT_BYTES,
            max_file_bytes: super::MAX_FILE_BYTES,
            jobs: 4,
            timeout: Duration::from_secs(120),
            path_filter: None,
            scope: Default::default(),
            exclude: Vec::new(),
            unstructured_documents: Default::default(),
            protected_paths: Vec::new(),
            include: None,
        }
    }
}

impl CaptureOptions {
    pub fn selector(&self) -> Result<impl Fn(&str) -> bool + Send + 'static> {
        let include = self
            .include
            .as_deref()
            .map(|patterns| compile(patterns, true))
            .transpose()?;
        let reason = self.exclusion_classifier()?;
        Ok(move |path: &str| {
            include.as_ref().is_none_or(|set| set.is_match(path)) && reason(path).is_none()
        })
    }

    pub fn exclusion_classifier(
        &self,
    ) -> Result<impl Fn(&str) -> Option<crate::domain::resource_exclusions::Reason> + Send + 'static>
    {
        let exclude = compile(&self.exclude, true)?;
        let retain = compile(&self.unstructured_documents.retain, true)?;
        let protected = compile(&self.protected_paths, false)?;
        // Bootstrap must acquire policy inputs even if their suffix is a resource suffix.
        let enabled = self.include.is_none() && self.unstructured_documents.enabled;
        Ok(move |path: &str| {
            crate::domain::resource_exclusions::reason(
                path,
                enabled,
                exclude.is_match(path),
                retain.is_match(path) || protected.is_match(path),
            )
        })
    }

    pub fn exclusion_evidence(&self) -> crate::domain::resource_exclusions::Evidence {
        crate::domain::resource_exclusions::Evidence {
            policy: self.unstructured_documents.clone(),
            protected_paths: self.protected_paths.clone(),
            ..Default::default()
        }
    }
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

fn compile(patterns: &[String], literal_separator: bool) -> Result<globset::GlobSet> {
    let mut builder = globset::GlobSetBuilder::new();
    for pattern in patterns {
        builder.add(
            globset::GlobBuilder::new(pattern)
                .literal_separator(literal_separator)
                .build()?,
        );
    }
    Ok(builder.build()?)
}
