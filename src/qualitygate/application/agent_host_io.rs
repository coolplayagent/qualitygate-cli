//! Bounded host I/O; called from the controller blocking worker.
use super::agent_host::Options;
use crate::{runner, snapshot};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{
    fmt,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

#[derive(Debug)]
pub(super) struct TimeBudgetExpired;

impl fmt::Display for TimeBudgetExpired {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Task time budget exhausted")
    }
}

impl std::error::Error for TimeBudgetExpired {}

pub(super) fn read(path: &Path, limit: usize) -> Result<Vec<u8>> {
    use std::io::Read;
    let metadata = std::fs::metadata(path)?;
    if !metadata.is_file() || metadata.len() > limit as u64 {
        bail!(
            "Input must be a regular file within {limit} bytes: {}",
            path.display()
        );
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        bail!("File exceeds {limit} byte budget: {}", path.display());
    }
    Ok(bytes)
}

pub(super) fn save(root: &Path, name: &str, bytes: &[u8]) -> Result<Value> {
    use std::io::Write;
    let mut temporary = tempfile::NamedTempFile::new_in(root)?;
    temporary.write_all(bytes)?;
    temporary.persist(root.join(name))?;
    Ok(json!({"path":name,"bytes":bytes.len(),"digest":snapshot::digest(bytes)}))
}

pub(super) async fn command(
    argv: &[String],
    root: &Path,
    input: Option<Vec<u8>>,
    deadline: Instant,
) -> Result<runner::Output> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err(TimeBudgetExpired.into());
    }
    tokio::select! {
        output = runner::capture(argv,root,input,remaining) => output,
        _ = tokio::signal::ctrl_c() => bail!("Interrupted; running process group cancelled"),
    }
}

pub(super) async fn resolve(root: &Path, reference: &str) -> Result<String> {
    let output = command(
        &[
            "git".into(),
            "rev-parse".into(),
            "--verify".into(),
            "--end-of-options".into(),
            format!("{reference}^{{commit}}"),
        ],
        root,
        None,
        Instant::now() + Duration::from_secs(15),
    )
    .await?;
    if output.exit_code != Some(0) || output.timed_out || output.capture_error.is_some() {
        bail!("Cannot resolve Git input {reference}");
    }
    let resolved = String::from_utf8(output.stdout)?.trim().to_string();
    if ![40, 64].contains(&resolved.len()) || !resolved.bytes().all(|c| c.is_ascii_hexdigit()) {
        bail!("Git did not return a full object identity");
    }
    Ok(resolved)
}

pub(super) async fn changes(options: &Options, evidence: &Path, label: &str) -> Result<Value> {
    // A separate, bounded cleanup window retains edits even after agent timeout.
    let deadline = Instant::now() + Duration::from_secs(10);
    let patch = command(
        &[
            "git".into(),
            "diff".into(),
            "--binary".into(),
            options.base.clone(),
        ],
        &options.root,
        None,
        deadline,
    )
    .await?;
    let patch_ref = save(evidence, &format!("{label}.patch"), &patch.stdout)?;
    if patch.exit_code != Some(0) || patch.timed_out || patch.capture_error.is_some() {
        bail!("Cannot completely preserve tracked changes");
    }
    let untracked = command(
        &[
            "git".into(),
            "ls-files".into(),
            "--others".into(),
            "--exclude-standard".into(),
            "-z".into(),
        ],
        &options.root,
        None,
        deadline,
    )
    .await?;
    if untracked.exit_code != Some(0) || untracked.timed_out || untracked.capture_error.is_some() {
        bail!("Cannot enumerate untracked changes");
    }
    let root = options.root.clone();
    let evidence = evidence.to_path_buf();
    let label = label.to_owned();
    tokio::task::spawn_blocking(move || -> Result<Value> {
        let mut files = Vec::new();
        let mut total = 0;
        for name in untracked
            .stdout
            .split(|byte| *byte == 0)
            .filter(|name| !name.is_empty())
        {
            if files.len() >= 1024 {
                bail!("Untracked file count exceeds 1024");
            }
            let name = std::str::from_utf8(name)?;
            let relative = Path::new(name);
            if relative
                .components()
                .any(|part| !matches!(part, std::path::Component::Normal(_)))
            {
                bail!("Untracked path is not confined");
            }
            let path = root.join(relative);
            let metadata = std::fs::symlink_metadata(&path)?;
            let (kind, bytes) = if metadata.file_type().is_symlink() {
                (
                    "symlink",
                    std::fs::read_link(&path)?
                        .into_os_string()
                        .into_string()
                        .map_err(|_| anyhow::anyhow!("Non-UTF-8 symlink target"))?
                        .into_bytes(),
                )
            } else {
                if !metadata.is_file() || !path.canonicalize()?.starts_with(&root) {
                    bail!("Untracked input is not a confined regular file");
                }
                ("file", read(&path, 2 * 1024 * 1024)?)
            };
            total += bytes.len();
            if total > runner::MAX_OUTPUT_BYTES {
                bail!("Untracked changes exceed 16 MiB");
            }
            let artifact = save(
                &evidence,
                &format!("{label}-untracked-{}", files.len()),
                &bytes,
            )?;
            #[cfg(unix)]
            let mode = {
                use std::os::unix::fs::PermissionsExt;
                Some(metadata.permissions().mode())
            };
            #[cfg(not(unix))]
            let mode: Option<u32> = None;
            files.push(json!({"path":name,"kind":kind,"mode":mode,"artifact":artifact}));
        }
        Ok(json!({"tracked_patch":patch_ref,"untracked":files}))
    })
    .await?
}

pub(super) fn replacement_target(root: &Path, relative: &Path) -> Result<PathBuf> {
    if !relative.starts_with("src")
        || relative.components().count() < 2
        || relative
            .components()
            .any(|part| !matches!(part, std::path::Component::Normal(_)))
    {
        bail!("Replacement target must be a relative file under src/");
    }
    let mut path = root.to_path_buf();
    for part in relative.components() {
        path.push(part);
        if std::fs::symlink_metadata(&path)?.file_type().is_symlink() {
            bail!("Replacement target cannot traverse symlinks");
        }
    }
    if !path.is_file() {
        bail!("Replacement target must be an existing regular file");
    }
    Ok(path)
}

pub(super) fn apply_replacement(
    root: &Path,
    relative: &Path,
    original: &[u8],
    message: &str,
) -> Result<Value> {
    use std::io::Write;
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Replacement {
        path: String,
        content: String,
    }
    let proposal: Replacement =
        serde_json::from_str(message).context("Agent did not return a replacement object")?;
    if Path::new(&proposal.path) != relative || proposal.content.len() > 1024 * 1024 {
        bail!("Agent replacement violates the fixed path or 1 MiB limit");
    }
    let path = replacement_target(root, relative)?;
    if read(&path, 1024 * 1024)? != original {
        bail!("Replacement input changed while agent was running");
    }
    let mut temporary = tempfile::NamedTempFile::new_in(path.parent().unwrap())?;
    temporary
        .as_file()
        .set_permissions(std::fs::metadata(&path)?.permissions())?;
    temporary.write_all(proposal.content.as_bytes())?;
    temporary.persist(&path)?;
    Ok(
        json!({"path":relative,"before":snapshot::digest(original),"after":snapshot::digest(proposal.content.as_bytes())}),
    )
}
