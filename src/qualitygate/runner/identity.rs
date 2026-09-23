//! Identify the executable selected from the command's actual working directory.

use crate::domain::Artifact;
use crate::domain::prerequisites::{FailureCode, Phase, PrerequisiteIssue};
use anyhow::{Result, bail};
use sha2::{Digest, Sha256};
use std::{io::Read, path::Path};

const MAX_EXECUTABLE_BYTES: u64 = 256 * 1024 * 1024;

pub async fn executable(program: &str, cwd: &Path) -> Result<Artifact> {
    let program = program.to_owned();
    let cwd = cwd.to_owned();
    tokio::task::spawn_blocking(move || {
        let workspace_issue = || {
            PrerequisiteIssue::new(
                FailureCode::WorkspaceUnavailable,
                Phase::Prepare,
                "Command working directory is unavailable",
            )
            .resource(cwd.display().to_string())
            .instruction(
                "Correct the configured cwd or ensure its prerequisite creates the directory.",
            )
        };
        if !std::fs::metadata(&cwd)
            .map_err(|error| workspace_issue().wrap(error.into()))?
            .is_dir()
        {
            return Err(workspace_issue().into());
        }
        let issue = || {
            PrerequisiteIssue::new(
                FailureCode::ToolUnavailable,
                Phase::Prepare,
                format!("Cannot resolve or identify executable {program}"),
            )
            .resource(program.clone())
            .instruction(
                "Install the configured tool or correct its executable path/PATH for this check.",
            )
        };
        let path = crate::env::executable(&program, &cwd).map_err(|error| issue().wrap(error))?;
        identify(&path, MAX_EXECUTABLE_BYTES).map_err(|error| issue().wrap(error))
    })
    .await?
}

/// Development evaluators can include large debug sections; this separate limit
/// does not relax the configured producer executable's 256 MiB boundary.
pub async fn evaluator() -> Result<Artifact> {
    tokio::task::spawn_blocking(|| identify(&crate::env::current_executable()?, 1024 * 1024 * 1024))
        .await?
}

fn identify(path: &Path, limit: u64) -> Result<Artifact> {
    let started = std::time::Instant::now();
    let file = std::fs::File::open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() > limit {
        bail!(
            "Executable exceeds its {} MiB identity budget: {}",
            limit / (1024 * 1024),
            path.display()
        );
    }
    let mut file = file.take(limit + 1);
    let mut hash = Sha256::new();
    let mut bytes = 0;
    let mut buffer = [0; 65_536];
    loop {
        if started.elapsed() > std::time::Duration::from_secs(30) {
            bail!("Executable identity exceeded its 30-second budget");
        }
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
        bytes += count as u64;
    }
    if bytes != metadata.len() || bytes > limit {
        bail!(
            "Executable changed while its identity was collected: {}",
            path.display()
        );
    }
    Ok(Artifact {
        path: path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Executable path is not UTF-8"))?
            .to_owned(),
        digest: format!("sha256:{:x}", hash.finalize()),
        bytes,
    })
}
