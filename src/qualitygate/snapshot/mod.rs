//! Immutable Git inputs and independently mapped change ranges.

mod changes;
mod git;
pub mod history;
mod input_guard;
mod limits;
mod merge_request;
mod worktree;
pub use changes::{Change, compare as compare_files};
pub use git::{read_commit, read_commit_with_options, resolve_commit, run_git};
pub use input_guard::InputGuard;
pub use limits::CaptureOptions;

pub use crate::domain::SnapshotIdentity as Identity;
use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const MAX_FILE_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_SNAPSHOT_BYTES: usize = 256 * 1024 * 1024;
pub const MAX_FILES: usize = 100_000;

#[derive(Debug, Clone)]
pub enum Selection {
    Worktree {
        base: String,
    },
    Staged,
    Diff {
        base: String,
        head: String,
    },
    Path {
        path: String,
        base: String,
    },
    MergeRequest {
        url: String,
        api_base: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub struct File {
    pub bytes: Vec<u8>,
    pub executable: bool,
}

#[derive(Debug, Clone)]
pub struct Snapshot {
    pub root: PathBuf,
    pub identity: Identity,
    pub files: BTreeMap<String, File>,
    pub base_files: BTreeMap<String, File>,
    pub changes: BTreeMap<String, Change>,
    pub path_filter: Option<String>,
    pub commits: Vec<(String, String)>,
}

pub fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

/// Includes path, executable bit and length framing, not only concatenated bytes.
pub fn content_digest(files: &BTreeMap<String, File>) -> String {
    content_digest_until(files, None).expect("hashing without a deadline cannot time out")
}

fn content_digest_until(
    files: &BTreeMap<String, File>,
    deadline: Option<std::time::Instant>,
) -> Result<String> {
    changes::check_deadline(deadline)?;
    let mut hash = Sha256::new();
    for (path, file) in files {
        changes::check_deadline(deadline)?;
        hash.update((path.len() as u64).to_le_bytes());
        hash.update(path.as_bytes());
        hash.update([u8::from(file.executable)]);
        hash.update((file.bytes.len() as u64).to_le_bytes());
        hash.update(&file.bytes);
    }
    changes::check_deadline(deadline)?;
    Ok(format!("sha256:{:x}", hash.finalize()))
}

pub async fn capture(root: &Path, selection: &Selection) -> Result<Snapshot> {
    capture_with_options(root, selection, &CaptureOptions::default()).await
}

pub async fn capture_with_options(
    root: &Path,
    selection: &Selection,
    options: &CaptureOptions,
) -> Result<Snapshot> {
    let acquisition = limits::Acquisition::new(options)?;
    tokio::time::timeout(
        options.timeout,
        capture_inner(root, selection, &acquisition),
    )
    .await
    .context("Snapshot acquisition timed out")?
}

async fn capture_inner(
    root: &Path,
    selection: &Selection,
    acquisition: &limits::Acquisition,
) -> Result<Snapshot> {
    let root_text = run_git(root, &["rev-parse", "--show-toplevel"], None).await?;
    let root = PathBuf::from(std::str::from_utf8(&root_text)?.trim());
    let head = resolve_commit(&root, "HEAD").await?;
    let merge_request = if let Selection::MergeRequest { url, api_base } = selection {
        Some(merge_request::resolve(&root, url, api_base.as_deref()).await?)
    } else {
        None
    };
    let (mode, base, target, path_filter) = match selection {
        Selection::Worktree { base } => ("worktree", base.as_str(), head.as_str(), None),
        Selection::Staged => ("staged", head.as_str(), head.as_str(), None),
        Selection::Diff { base, head } => ("diff", base.as_str(), head.as_str(), None),
        Selection::Path { path, base } => (
            "path",
            base.as_str(),
            head.as_str(),
            Some(crate::paths::relative(Path::new(path))?),
        ),
        Selection::MergeRequest { .. } => {
            let comparison = merge_request.as_ref().expect("resolved MR selection");
            (
                "mr",
                comparison.merge_base.as_str(),
                comparison.source_head.as_str(),
                None,
            )
        }
    };
    let base = resolve_commit(&root, base).await?;
    let target = resolve_commit(&root, target).await?;
    let path_filter = acquisition
        .options
        .path_filter
        .as_ref()
        .map(|path| crate::paths::relative(Path::new(path)))
        .transpose()?
        .or(path_filter);
    let read_target = async {
        match selection {
            Selection::Diff { .. } | Selection::MergeRequest { .. } => {
                git::commit(&root, &target, acquisition).await
            }
            Selection::Staged => git::read_index(&root, acquisition).await,
            _ => worktree::read(&root, acquisition).await,
        }
    };
    let (base_files, files, commits) = tokio::try_join!(
        git::commit(&root, &base, acquisition),
        read_target,
        git::messages(&root, &base, &target)
    )?;
    // Diffing and hashing scale with snapshot contents and must not occupy an async worker.
    let deadline = Some(acquisition.deadline);
    tokio::task::spawn_blocking(move || {
        let changes = changes::compare_until(&base_files, &files, deadline)?;
        Ok(Snapshot {
            root,
            identity: Identity {
                mode: mode.into(),
                base,
                head: target,
                content_digest: content_digest_until(&files, deadline)?,
                merge_request,
            },
            files,
            base_files,
            changes,
            path_filter,
            commits,
        })
    })
    .await?
}

/// Owns the disposable directory while exposing its physical execution path.
pub struct Materialized {
    _directory: tempfile::TempDir,
    root: PathBuf,
}

impl Materialized {
    fn new(directory: tempfile::TempDir) -> Result<Self> {
        let root = dunce::canonicalize(directory.path())?;
        Ok(Self {
            _directory: directory,
            root,
        })
    }

    pub fn path(&self) -> &Path {
        &self.root
    }
}

/// Materializes checked bytes without exposing the user's working tree to tools.
pub async fn materialize(snapshot: &Snapshot) -> Result<Materialized> {
    let files = snapshot.files.clone();
    tokio::task::spawn_blocking(move || materialize_files(&files)).await?
}

/// Concurrent evaluators share immutable bytes instead of cloning the entire source tree.
pub async fn materialize_shared(snapshot: std::sync::Arc<Snapshot>) -> Result<Materialized> {
    tokio::task::spawn_blocking(move || materialize_files(&snapshot.files)).await?
}

fn materialize_files(files: &BTreeMap<String, File>) -> Result<Materialized> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let directory = Materialized::new(
        tempfile::Builder::new()
            .prefix("qualitygate-snapshot-")
            .tempdir()?,
    )?;
    for (name, file) in files {
        if std::time::Instant::now() >= deadline {
            anyhow::bail!("Snapshot materialization exceeded its 30-second budget");
        }
        let path = crate::paths::confined(directory.path(), Path::new(name))?;
        std::fs::create_dir_all(path.parent().context("File has no parent")?)?;
        std::fs::write(&path, &file.bytes)?;
        #[cfg(unix)]
        if file.executable {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
        }
    }
    Ok(directory)
}

impl Snapshot {
    pub fn includes(&self, file: &str) -> bool {
        self.path_filter.as_ref().is_none_or(|filter| {
            file == filter
                || file
                    .strip_prefix(filter)
                    .is_some_and(|tail| tail.starts_with('/'))
        })
    }
}

#[cfg(test)]
#[path = "snapshot_tests.rs"]
mod tests;
