//! Immutable Git inputs and independently mapped change ranges.

mod changes;
mod git;
pub use changes::Change;
pub use git::{read_commit, resolve_commit, run_git};

pub use crate::domain::SnapshotIdentity as Identity;
use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const MAX_FILE_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_SNAPSHOT_BYTES: usize = 12 * 1024 * 1024;
pub const MAX_FILES: usize = 20_000;

#[derive(Debug, Clone)]
pub enum Selection {
    Worktree { base: String },
    Staged,
    Diff { base: String, head: String },
    Path { path: String, base: String },
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
    let mut hash = Sha256::new();
    for (path, file) in files {
        hash.update((path.len() as u64).to_le_bytes());
        hash.update(path.as_bytes());
        hash.update([u8::from(file.executable)]);
        hash.update((file.bytes.len() as u64).to_le_bytes());
        hash.update(&file.bytes);
    }
    format!("sha256:{:x}", hash.finalize())
}

pub async fn capture(root: &Path, selection: &Selection) -> Result<Snapshot> {
    let root_text = run_git(root, &["rev-parse", "--show-toplevel"], None).await?;
    let root = PathBuf::from(std::str::from_utf8(&root_text)?.trim());
    let head = resolve_commit(&root, "HEAD").await?;
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
    };
    let base = resolve_commit(&root, base).await?;
    let target = resolve_commit(&root, target).await?;
    let base_files = read_commit(&root, &base).await?;
    let files = match selection {
        Selection::Diff { .. } => read_commit(&root, &target).await?,
        Selection::Staged => git::read_index(&root).await?,
        _ => read_worktree(&root).await?,
    };
    let commits = git::messages(&root, &base, &target).await?;
    let changes = changes::compare(&base_files, &files);
    Ok(Snapshot {
        root,
        identity: Identity {
            mode: mode.into(),
            base,
            head: target,
            content_digest: content_digest(&files),
        },
        files,
        base_files,
        changes,
        path_filter,
        commits,
    })
}

async fn read_worktree(root: &Path) -> Result<BTreeMap<String, File>> {
    let listing = run_git(
        root,
        &[
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
        ],
        None,
    )
    .await?;
    let names: std::collections::BTreeSet<String> = listing
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
        .map(|entry| String::from_utf8(entry.to_vec()))
        .collect::<std::result::Result<_, _>>()?;
    if names.len() > MAX_FILES {
        bail!("Snapshot exceeds {MAX_FILES} files");
    }
    let root = root.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let mut files = BTreeMap::new();
        let mut total = 0;
        for name in names {
            let path = crate::paths::confined(&root, Path::new(&name))?;
            let metadata = match std::fs::metadata(&path) {
                Ok(value) => value,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error.into()),
            };
            if !metadata.is_file() {
                bail!("Unsupported snapshot entry (including submodule): {name}");
            }
            if metadata.len() > MAX_FILE_BYTES as u64 {
                bail!("File exceeds {MAX_FILE_BYTES} bytes: {name}");
            }
            let bytes = std::fs::read(&path)?;
            total += bytes.len();
            if total > MAX_SNAPSHOT_BYTES || bytes.len() > MAX_FILE_BYTES {
                bail!("Snapshot input budget exceeded at {name}");
            }
            #[cfg(unix)]
            let executable = {
                use std::os::unix::fs::PermissionsExt;
                metadata.permissions().mode() & 0o111 != 0
            };
            #[cfg(not(unix))]
            let executable = false;
            files.insert(name, File { bytes, executable });
        }
        Ok(files)
    })
    .await?
}

/// Materializes checked bytes without exposing the user's working tree to tools.
pub async fn materialize(snapshot: &Snapshot) -> Result<tempfile::TempDir> {
    let files = snapshot.files.clone();
    tokio::task::spawn_blocking(move || {
        let directory = tempfile::Builder::new()
            .prefix("qualitygate-snapshot-")
            .tempdir()?;
        for (name, file) in files {
            let path = crate::paths::confined(directory.path(), Path::new(&name))?;
            std::fs::create_dir_all(path.parent().context("File has no parent")?)?;
            std::fs::write(&path, file.bytes)?;
            #[cfg(unix)]
            if file.executable {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
            }
        }
        Ok(directory)
    })
    .await?
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
