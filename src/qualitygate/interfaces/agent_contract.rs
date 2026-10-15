use crate::{application::CheckOptions, snapshot::Selection};
use anyhow::Result;
use clap::Args;
use std::path::PathBuf;

#[derive(Debug, Args)]
#[group(skip)]
pub(super) struct ProjectArgs {
    #[arg(long, group = "selection")]
    diff: Option<String>,
    #[arg(long, group = "selection")]
    staged: bool,
    #[arg(long, group = "selection")]
    worktree: bool,
    #[arg(long, conflicts_with_all = ["diff", "staged"])]
    base: Option<String>,
    #[arg(long)]
    task: Option<String>,
    #[arg(long)]
    policy_ref: Option<String>,
    #[arg(long, default_value = "implementer")]
    pub role: String,
    #[command(flatten)]
    snapshot: super::doctor::SnapshotArgs,
}

impl ProjectArgs {
    pub fn options(self, root: PathBuf, config: String) -> Result<CheckOptions> {
        let selection = if let Some(diff) = self.diff {
            super::doctor::diff_selection(&diff)?
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
            profile: "full".into(),
            task: self.task,
            policy_ref: self.policy_ref,
            output_dir: None,
            trust_store: None,
            evidence_dir: None,
        })
    }
}
