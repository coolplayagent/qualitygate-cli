//! Immutable Git inputs and independently mapped change ranges.

mod changes;
mod git;
pub mod history;
mod input_guard;
mod io_workers;
mod limits;
mod merge_request;
pub mod preflight;
pub mod test_overlay;
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

pub const MAX_FILE_BYTES: usize = crate::domain::snapshot_budget::DEFAULT_FILE_BYTES;
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
    pub scope_evidence: crate::domain::check_scope::ScopeEvidence,
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

/// Derived execution inputs retain their parent's scope binding as well as bytes.
pub fn bind_derived(identity: &mut Identity, context_digest: String) {
    if let Some(parent) = &identity.verification_digest {
        identity.verification_digest = Some(digest(&serde_json::to_vec(&serde_json::json!({
            "parent":parent,"mode":identity.mode,"base":identity.base,"head":identity.head,"context":context_digest
        })).expect("serializable derived identity")));
    }
    identity.content_digest = context_digest;
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
    capture_resolved(root, selection, options, None).await
}

pub async fn capture_resolved(
    root: &Path,
    selection: &Selection,
    options: &CaptureOptions,
    comparison: Option<crate::domain::MergeRequest>,
) -> Result<Snapshot> {
    let acquisition = limits::Acquisition::new(options)?;
    tokio::time::timeout(
        options.timeout,
        capture_inner(root, selection, &acquisition, comparison),
    )
    .await
    .context("Snapshot acquisition timed out")
    .and_then(|result| result)
    .map_err(|error| acquisition_error(error, root.display().to_string()))
}

fn acquisition_error(error: anyhow::Error, resource: String) -> anyhow::Error {
    if error
        .downcast_ref::<crate::domain::prerequisites::PrerequisiteIssue>()
        .is_some()
    {
        return error;
    }
    let mut issue = snapshot_issue().resource(resource);
    issue.message = format!("{error:#}");
    issue.into()
}

fn snapshot_issue() -> crate::domain::prerequisites::PrerequisiteIssue {
    crate::domain::prerequisites::PrerequisiteIssue::new(
        crate::domain::prerequisites::FailureCode::SnapshotUnavailable,
        crate::domain::prerequisites::Phase::Inputs, "Cannot acquire the selected snapshot")
        .instruction("Inspect the reported paths, conflicts and acquisition budgets; repair the selected inputs or adjust supported caller budgets.")
}

async fn capture_inner(
    root: &Path,
    selection: &Selection,
    acquisition: &limits::Acquisition,
    comparison: Option<crate::domain::MergeRequest>,
) -> Result<Snapshot> {
    let root_text = run_git(root, &["rev-parse", "--show-toplevel"], None).await?;
    let root = PathBuf::from(std::str::from_utf8(&root_text)?.trim());
    let head = resolve_commit(&root, "HEAD").await?;
    let merge_request = if let Selection::MergeRequest { url, api_base } = selection {
        Some(match comparison {
            Some(comparison) => comparison,
            None => merge_request::resolve(&root, url, api_base.as_deref()).await?,
        })
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
    let scope = acquisition.options.scope;
    let exclude = acquisition.options.exclude.clone();
    let excluded_paths = if exclude.is_empty() {
        Vec::new()
    } else {
        let selected = acquisition.options.selector()?;
        let mut paths = std::collections::BTreeSet::new();
        for reference in [&base, &target] {
            let names = run_git(
                &root,
                &["ls-tree", "-r", "--name-only", "-z", reference],
                None,
            )
            .await?;
            for name in names.split(|b| *b == 0).filter(|p| !p.is_empty()) {
                let path = std::str::from_utf8(name)?;
                if !selected(path) {
                    paths.insert(path.to_owned());
                }
            }
        }
        if matches!(
            selection,
            Selection::Worktree { .. } | Selection::Path { .. } | Selection::Staged
        ) {
            let args: &[&str] = if matches!(selection, Selection::Staged) {
                &["ls-files", "--cached", "-z"]
            } else {
                &[
                    "ls-files",
                    "--cached",
                    "--others",
                    "--exclude-standard",
                    "-z",
                ]
            };
            let names = run_git(&root, args, None).await?;
            for name in names.split(|b| *b == 0).filter(|p| !p.is_empty()) {
                let path = std::str::from_utf8(name)?;
                if !selected(path) {
                    paths.insert(path.to_owned());
                }
            }
        }
        anyhow::ensure!(
            paths.len() <= MAX_FILES,
            "Excluded inventory exceeds file budget"
        );
        paths.into_iter().collect()
    };
    // Diffing and hashing scale with snapshot contents and must not occupy an async worker.
    let deadline = Some(acquisition.deadline);
    tokio::task::spawn_blocking(move || {
        let changes = changes::compare_until(&base_files, &files, deadline)?;
        let context_digest = content_digest_until(&files, deadline)?;
        let scope_evidence = crate::domain::check_scope::ScopeEvidence {
            mode: scope, exclude, excluded_paths,
            changed_files: changes.keys().cloned().collect(),
            changed_lines: changes.values().map(|change| change.added_lines.len()).sum(),
            execution_context_digest: context_digest.clone(), empty_delivery: changes.is_empty(),
        };
        let verification_digest = if scope == crate::domain::check_scope::CheckScope::Repository && scope_evidence.exclude.is_empty() && path_filter.is_none() {
            None
        } else {
            Some(digest(&serde_json::to_vec(&serde_json::json!({"context":context_digest,"base_context":content_digest_until(&base_files, deadline)?,"scope":scope_evidence,"path_filter":path_filter,"changes":changes}))?))
        };
        Ok(Snapshot {
            scope_evidence,
            root,
            identity: Identity {
                mode: mode.into(),
                base,
                head: target,
                content_digest: context_digest,
                verification_digest,
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
    tokio::task::spawn_blocking(move || materialize_files(&files))
        .await?
        .map_err(workspace_error)
}

/// Concurrent evaluators share immutable bytes instead of cloning the entire source tree.
pub async fn materialize_shared(snapshot: std::sync::Arc<Snapshot>) -> Result<Materialized> {
    tokio::task::spawn_blocking(move || materialize_files(&snapshot.files))
        .await?
        .map_err(workspace_error)
}

fn workspace_error(error: anyhow::Error) -> anyhow::Error {
    crate::domain::prerequisites::PrerequisiteIssue::new(
        crate::domain::prerequisites::FailureCode::WorkspaceUnavailable,
        crate::domain::prerequisites::Phase::Prepare,
        "Cannot materialize the execution workspace",
    )
    .instruction("Check temporary storage access and capacity, then retry.")
    .wrap(error)
}

fn materialize_files(files: &BTreeMap<String, File>) -> Result<Materialized> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let timeout = "Snapshot materialization exceeded its 30-second budget";
    if files.len() > MAX_FILES {
        anyhow::bail!("Snapshot I/O exceeds the file-count limit");
    }
    let directory = Materialized::new(
        tempfile::Builder::new()
            .prefix("qualitygate-snapshot-")
            .tempdir()?,
    )?;
    let mut parents = std::collections::BTreeSet::new();
    for name in files.keys() {
        if std::time::Instant::now() >= deadline {
            anyhow::bail!("{timeout}");
        }
        crate::paths::relative(Path::new(name))?;
        parents.insert(Path::new(name).parent().context("File has no parent")?);
    }
    for parent in parents {
        if std::time::Instant::now() >= deadline {
            anyhow::bail!("{timeout}");
        }
        std::fs::create_dir_all(crate::paths::confined(directory.path(), parent)?)?;
    }
    io_workers::map(files, deadline, timeout, |name, file| {
        use std::io::Write;
        // Names and parents were confined above. This fresh workspace remains
        // private until all writers join; no command can create symlink parents.
        // Exclusive creation also rejects leaf symlinks and native path aliases.
        let path = directory.path().join(name);
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?
            .write_all(&file.bytes)?;
        #[cfg(unix)]
        if file.executable {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
        }
        Ok(())
    })?;
    Ok(directory)
}

impl Snapshot {
    pub fn delivery(&self) -> bool {
        self.scope_evidence.mode == crate::domain::check_scope::CheckScope::Delivery
    }

    pub fn selects_diagnostic(
        &self,
        file: Option<&str>,
        range: Option<&crate::domain::Range>,
    ) -> bool {
        let Some(file) = file else {
            return true;
        };
        if !self.includes(file) {
            return false;
        }
        if !self.delivery() {
            return true;
        }
        let Some(change) = self.changes.get(file).or_else(|| {
            self.changes
                .values()
                .find(|c| c.old_path.as_deref() == Some(file))
        }) else {
            return false;
        };
        range.is_none_or(|range| {
            change
                .added_lines
                .range(range.start_line..=range.end_line)
                .next()
                .is_some()
        })
    }

    pub fn includes(&self, file: &str) -> bool {
        if self.delivery()
            && !self.changes.contains_key(file)
            && !self
                .changes
                .values()
                .any(|change| change.old_path.as_deref() == Some(file))
        {
            return false;
        }
        self.feedback_includes(file)
    }

    pub fn feedback_includes(&self, file: &str) -> bool {
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
