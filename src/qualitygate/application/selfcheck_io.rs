//! Native fixture boundaries: disposable Git objects, confined I/O and processes.

use crate::{
    config::selfcheck::{RunnerScenario, SnapshotScenario},
    snapshot::{self, Selection},
};
use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::{path::Path, time::Duration};

async fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    snapshot::run_git(root, args, None).await
}

pub(super) async fn snapshot(scenario: SnapshotScenario) -> Result<Value> {
    crate::env::require_isolated_git_environment()?;
    // This function is driven by a blocking fixture worker; filesystem setup
    // stays outside the runtime's orchestration threads.
    let directory = tempfile::tempdir().context("Cannot create snapshot fixture directory")?;
    let root = dunce::canonicalize(directory.path())?;
    git(&root, &["init", "-q", "--template="]).await?;
    git(&root, &["config", "core.autocrlf", "false"]).await?;
    git(&root, &["config", "core.safecrlf", "false"]).await?;
    git(&root, &["config", "core.longpaths", "true"]).await?;
    git(
        &root,
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=",
            "commit",
            "--allow-empty",
            "-qm",
            "[fixture]test: base",
        ],
    )
    .await?;
    let path = if matches!(scenario, SnapshotScenario::LongPath) {
        format!(
            "{}/特殊 空间!.txt",
            "nested-directory/".repeat(20).trim_end_matches('/')
        )
    } else {
        "fixture.txt".into()
    };
    let file = root.join(&path);
    std::fs::create_dir_all(file.parent().context("Fixture file has no parent")?)?;
    std::fs::write(&file, b"staged\n")?;
    let selection = if matches!(
        scenario,
        SnapshotScenario::Staged | SnapshotScenario::Symlink | SnapshotScenario::MissingObject
    ) {
        git(&root, &["add", "--", &path]).await?;
        Selection::Staged
    } else {
        Selection::Worktree {
            base: "HEAD".into(),
        }
    };
    match scenario {
        SnapshotScenario::Staged => std::fs::write(&file, b"worktree differs\n")?,
        SnapshotScenario::Oversized => {
            std::fs::write(&file, vec![b'x'; snapshot::MAX_FILE_BYTES + 1])?
        }
        SnapshotScenario::Symlink => {
            let oid = git(&root, &["hash-object", "-w", "--", &path]).await?;
            let entry = format!("120000,{},{}", std::str::from_utf8(&oid)?.trim(), path);
            git(&root, &["update-index", "--add", "--cacheinfo", &entry]).await?;
        }
        SnapshotScenario::MissingObject => {
            let oid = git(&root, &["hash-object", "--", &path]).await?;
            let oid = std::str::from_utf8(&oid)?.trim();
            // Only this disposable repository's freshly written loose object.
            let object = root.join(".git/objects").join(&oid[..2]).join(&oid[2..]);
            std::fs::rename(object, root.join("withheld-object"))?;
        }
        _ => {}
    }
    let observed = match snapshot::capture(&root, &selection).await {
        Ok(captured) if matches!(scenario, SnapshotScenario::ChangedInput) => {
            let guard = snapshot::InputGuard::new(&root, captured.files).await?;
            std::fs::write(&file, b"changed\n")?;
            match guard.verify().await {
                Ok(()) => json!({"status":"completed"}),
                Err(error) => json!({"status":"incomplete", "reason":format!("{error:#}")}),
            }
        }
        Ok(captured) => {
            json!({"status":"completed", "mode":captured.identity.mode, "files":captured.files.iter().map(|(path, file)| (path.clone(), String::from_utf8_lossy(&file.bytes).into_owned())).collect::<std::collections::BTreeMap<_,_>>() })
        }
        Err(error) => json!({"status":"incomplete", "reason":format!("{error:#}")}),
    };
    Ok(observed)
}

pub(super) async fn runner(scenario: RunnerScenario) -> Result<Value> {
    let directory = tempfile::tempdir().context("Cannot create runner fixture directory")?;
    let mode = match scenario {
        RunnerScenario::Success => "success",
        RunnerScenario::Failure => "failure",
        RunnerScenario::Timeout => "timeout",
        RunnerScenario::Overflow => "overflow",
        RunnerScenario::MissingTool => "missing",
    };
    let executable = if matches!(scenario, RunnerScenario::MissingTool) {
        directory.path().join("missing-fixture-tool")
    } else {
        crate::env::current_executable()?
    };
    let argv = vec![
        executable.to_string_lossy().into_owned(),
        "selfcheck-probe".into(),
        mode.into(),
    ];
    let deadline = if matches!(scenario, RunnerScenario::Timeout) {
        Duration::from_millis(150)
    } else {
        Duration::from_secs(10)
    };
    Ok(
        match crate::runner::capture(&argv, directory.path(), None, deadline).await {
            Ok(output) => {
                json!({"status":if output.timed_out || output.capture_error.is_some() { "incomplete" } else { "completed" }, "timed_out":output.timed_out, "exit_code":output.exit_code, "capture_error":output.capture_error, "stdout_bytes":output.stdout.len()})
            }
            Err(error) => json!({"status":"incomplete", "reason":format!("{error:#}")}),
        },
    )
}
