use crate::{
    application::CheckOptions,
    snapshot::{CaptureOptions, Selection},
};
use anyhow::{Context, Result, ensure};
use clap::Args;
use std::{path::PathBuf, time::Duration};

#[derive(Debug, Args)]
#[group(skip)]
pub(super) struct DoctorArgs {
    #[arg(long, group = "selection")]
    diff: Option<String>,
    #[arg(long, group = "selection")]
    staged: bool,
    #[arg(long, group = "selection")]
    worktree: bool,
    #[arg(long, conflicts_with_all = ["diff", "staged"])]
    base: Option<String>,
    #[arg(long, default_value = "full", value_parser = ["quick", "full"])]
    pub profile: String,
    #[arg(long)]
    task: Option<String>,
    #[arg(long)]
    policy_ref: Option<String>,
    /// Execute only declared tool version probes; never formal check commands.
    #[arg(long)]
    pub probe_tools: bool,
    #[command(flatten)]
    snapshot: SnapshotArgs,
}

#[derive(Debug, Args)]
#[group(skip)]
pub(super) struct SnapshotArgs {
    /// Maximum total content bytes per snapshot, in MiB (1..=1024).
    #[arg(long, default_value_t = 256, value_parser = clap::value_parser!(u32).range(1..=1024))]
    pub snapshot_max_mib: u32,
    /// Maximum bytes per snapshot file, in MiB (1..=8); independent of rule scope.
    #[arg(long, default_value_t = 2, value_parser = clap::value_parser!(u32).range(1..=8))]
    pub snapshot_max_file_mib: u32,
    /// Shared maximum concurrent snapshot content readers (1..=16).
    #[arg(long, default_value_t = 4, value_parser = clap::value_parser!(u32).range(1..=16))]
    pub snapshot_jobs: u32,
    /// Deadline for each complete snapshot acquisition (1..=3600 seconds).
    #[arg(long, default_value_t = 120, value_parser = clap::value_parser!(u32).range(1..=3600))]
    pub snapshot_timeout_secs: u32,
}

impl SnapshotArgs {
    pub fn options(&self) -> CaptureOptions {
        CaptureOptions {
            scope: crate::domain::check_scope::CheckScope::Delivery,
            max_bytes: self.snapshot_max_mib as usize * 1024 * 1024,
            max_file_bytes: self.snapshot_max_file_mib as usize * 1024 * 1024,
            jobs: self.snapshot_jobs as usize,
            timeout: Duration::from_secs(self.snapshot_timeout_secs.into()),
            ..Default::default()
        }
    }
}

pub(super) fn diff_selection(diff: &str) -> Result<Selection> {
    let (base, head) = diff
        .split_once("..")
        .context("--diff requires <base>..<head>")?;
    ensure!(
        !base.is_empty() && !head.is_empty() && !head.starts_with('.'),
        "--diff requires two explicit commit endpoints"
    );
    Ok(Selection::Diff {
        base: base.into(),
        head: head.into(),
    })
}

impl DoctorArgs {
    pub fn options(self, root: PathBuf, config: String) -> Result<CheckOptions> {
        let selection = if let Some(diff) = self.diff {
            diff_selection(&diff)?
        } else if self.staged {
            Selection::Staged
        } else {
            Selection::Worktree {
                base: self.base.unwrap_or_else(|| "HEAD".into()),
            }
        };
        Ok(CheckOptions {
            root,
            config,
            selection,
            snapshot_options: self.snapshot.options(),
            profile: self.profile,
            task: self.task,
            policy_ref: self.policy_ref,
            output_dir: None,
            trust_store: None,
            evidence_dir: None,
        })
    }
}
