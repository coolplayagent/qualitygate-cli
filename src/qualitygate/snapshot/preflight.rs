//! Advisory metadata inspection; no source content is read or excluded from checks.

use super::{MAX_FILES, resolve_commit, run_git};
use crate::domain::snapshot_budget::{
    DEFAULT_FILE_BYTES, MAX_FILE_BYTES, OversizedFile, Preflight,
};
use anyhow::{Context, Result, bail};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    time::{Duration, Instant},
};

pub async fn inspect(root: &Path) -> Result<Preflight> {
    tokio::time::timeout(Duration::from_secs(30), inspect_inner(root))
        .await
        .context("Snapshot preflight exceeded its 30-second budget")?
}

async fn inspect_inner(root: &Path) -> Result<Preflight> {
    let deadline = Instant::now() + Duration::from_secs(30);
    let root = run_git(root, &["rev-parse", "--show-toplevel"], None).await?;
    let root = std::path::PathBuf::from(std::str::from_utf8(&root)?.trim());
    let head = resolve_commit(&root, "HEAD").await?;
    let tree = run_git(&root, &["ls-tree", "-r", "-l", "-z", &head], None).await?;
    let names = run_git(
        &root,
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
    tokio::task::spawn_blocking(move || {
        let mut sizes = BTreeMap::<String, u64>::new();
        let mut unsupported = BTreeSet::new();
        for (index, entry) in tree.split(|byte| *byte == 0).filter(|entry| !entry.is_empty()).enumerate() {
            check_budget(index, deadline)?;
            let (metadata, path) = std::str::from_utf8(entry)?.split_once('\t').context("Malformed preflight Git entry")?;
            let fields: Vec<_> = metadata.split_whitespace().collect();
            if fields.len() != 4 { bail!("Malformed preflight Git metadata"); }
            if ["100644", "100755"].contains(&fields[0]) {
                sizes.insert(path.to_owned(), fields[3].parse()?);
            } else {
                unsupported.insert(format!("{path} (Git mode {})", fields[0]));
            }
        }
        for (index, name) in names.split(|byte| *byte == 0).filter(|name| !name.is_empty()).enumerate() {
            check_budget(index, deadline)?;
            let name = std::str::from_utf8(name)?;
            let path = match crate::paths::confined(&root, Path::new(name)) {
                Ok(path) => path,
                Err(error) => { unsupported.insert(format!("{name} ({error})")); continue; }
            };
            let metadata = match std::fs::symlink_metadata(&path) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error.into()),
            };
            if !metadata.is_file() {
                unsupported.insert(format!("{name} (non-regular worktree entry)"));
            } else {
                let size = sizes.entry(name.to_owned()).or_default();
                *size = (*size).max(metadata.len());
            }
        }
        let oversized: Vec<_> = sizes.into_iter().filter(|(_, size)| *size > DEFAULT_FILE_BYTES as u64)
            .map(|(path, bytes)| OversizedFile { path, bytes }).collect();
        let maximum = oversized.iter().map(|file| file.bytes).max().unwrap_or_default();
        let recommended = (maximum > 0 && maximum <= MAX_FILE_BYTES as u64)
            .then(|| maximum.div_ceil(1024 * 1024));
        let mut next_steps = vec![
            "Review detected languages and suggested commands before adopting the candidate.".into(),
            "--diff/--mr compare complete snapshots; --path filters feedback, not acquisition. Rule path/language filters do not exclude snapshot bytes.".into(),
            "This HEAD/worktree metadata preflight is advisory, not a gate: selected refs/index, total budgets, content readability and tools still require check.".into(),
        ];
        if let Some(mib) = recommended {
            next_steps.insert(0, format!("Run: qualitygate check --profile full --snapshot-max-file-mib {mib} (use the same --root and --config); this changes acquisition capacity, not repository policy."));
        } else if maximum > MAX_FILE_BYTES as u64 {
            next_steps.insert(0, "Snapshot contains files above the supported 8 MiB single-file limit; acquisition remains incomplete. No ignore or severity setting can make this a complete check.".into());
        }
        if !unsupported.is_empty() {
            next_steps.insert(0, "Unsupported Git/worktree entries prevent capture; review the listed paths before checking.".into());
        }
        Ok(Preflight {
            scope: "HEAD and eligible worktree metadata", head, complete: true,
            default_max_file_bytes: DEFAULT_FILE_BYTES, maximum_max_file_bytes: MAX_FILE_BYTES,
            oversized_file_count: oversized.len(), unsupported_entry_count: unsupported.len(),
            details_truncated: oversized.len() > 100 || unsupported.len() > 100,
            oversized_files: oversized.into_iter().take(100).collect(),
            unsupported_entries: unsupported.into_iter().take(100).collect(),
            recommended_max_file_mib: recommended, next_steps,
        })
    }).await?
}

fn check_budget(index: usize, deadline: Instant) -> Result<()> {
    if index >= MAX_FILES || Instant::now() >= deadline {
        bail!("Snapshot preflight exceeds 100000 entries or its 30-second budget");
    }
    Ok(())
}
