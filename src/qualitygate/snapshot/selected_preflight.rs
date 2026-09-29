//! Metadata-only budgets for the exact base/target used by snapshot capture.

use super::{CaptureOptions, Selection, resolve_commit, run_git};
use crate::domain::{
    doctor::{DETAIL_LIMIT, SnapshotBudget},
    snapshot_budget::{MAX_FILE_BYTES, OversizedFile},
};
use anyhow::{Context, Result, bail, ensure};
use serde::Serialize;
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    time::Instant,
};

#[derive(Serialize)]
struct Entry {
    path: String,
    bytes: u64,
    identity: String,
    unsupported: Option<String>,
}

pub async fn inspect(
    root: &Path,
    selection: &Selection,
    options: &CaptureOptions,
    protected: &[String],
    policy_ref: Option<&str>,
) -> Result<Vec<SnapshotBudget>> {
    options.validate()?;
    tokio::time::timeout(
        options.timeout,
        inspect_inner(root, selection, options, protected, policy_ref),
    )
    .await
    .context("Selected metadata preflight timed out")?
}

async fn inspect_inner(
    root: &Path,
    selection: &Selection,
    options: &CaptureOptions,
    protected: &[String],
    policy_ref: Option<&str>,
) -> Result<Vec<SnapshotBudget>> {
    let deadline = Instant::now() + options.timeout;
    let output = run_git(root, &["rev-parse", "--show-toplevel"], None).await?;
    let root = PathBuf::from(std::str::from_utf8(&output)?.trim());
    let head = resolve_commit(&root, "HEAD").await?;
    let base = match selection {
        Selection::Worktree { base } | Selection::Diff { base, .. } => {
            resolve_commit(&root, base).await?
        }
        Selection::Staged => head.clone(),
        _ => bail!("doctor supports worktree, staged and diff snapshots only"),
    };
    let base_entries = commit(&root, &base, deadline).await?;
    let (target, target_entries) = match selection {
        Selection::Diff { head, .. } => {
            let target = resolve_commit(&root, head).await?;
            let entries = commit(&root, &target, deadline).await?;
            (target, entries)
        }
        Selection::Staged => (head, index(&root, deadline).await?),
        _ => (head, worktree(&root, deadline).await?),
    };
    let mut inventories = vec![
        ("base", base, base_entries),
        ("target", target, target_entries),
    ];
    if let Some(reference) = policy_ref {
        let reference = resolve_commit(&root, reference).await?;
        inventories.push((
            "policy",
            reference.clone(),
            commit(&root, &reference, deadline).await?,
        ));
    }
    let options = options.clone();
    let protected = protected.to_vec();
    tokio::task::spawn_blocking(move || {
        inventories
            .into_iter()
            .map(|(side, reference, entries)| {
                summarize(side, reference, entries, &options, &protected, deadline)
            })
            .collect()
    })
    .await?
}

async fn commit(root: &Path, reference: &str, deadline: Instant) -> Result<Vec<Entry>> {
    let output = run_git(root, &["ls-tree", "-r", "-l", "-z", reference], None).await?;
    tokio::task::spawn_blocking(move || -> Result<_> {
        output
            .split(|byte| *byte == 0)
            .filter(|entry| !entry.is_empty())
            .map(|entry| {
                budget(deadline)?;
                let (metadata, path) = std::str::from_utf8(entry)?
                    .split_once('\t')
                    .context("Malformed Git metadata")?;
                let fields: Vec<_> = metadata.split_whitespace().collect();
                ensure!(fields.len() == 4, "Malformed Git metadata fields");
                let unsupported = (!matches!(fields[0], "100644" | "100755"))
                    .then(|| format!("Git mode {}", fields[0]));
                Ok(Entry {
                    path: path.into(),
                    bytes: if fields[3] == "-" {
                        0
                    } else {
                        fields[3].parse()?
                    },
                    identity: format!("{}:{}", fields[0], fields[2]),
                    unsupported,
                })
            })
            .collect()
    })
    .await?
}

async fn index(root: &Path, deadline: Instant) -> Result<Vec<Entry>> {
    let output = run_git(root, &["ls-files", "--stage", "-z"], None).await?;
    let entries = tokio::task::spawn_blocking(move || -> Result<Vec<_>> {
        output
            .split(|byte| *byte == 0)
            .filter(|entry| !entry.is_empty())
            .map(|entry| {
                budget(deadline)?;
                let (metadata, path) = std::str::from_utf8(entry)?
                    .split_once('\t')
                    .context("Malformed index metadata")?;
                let fields: Vec<_> = metadata.split_whitespace().collect();
                ensure!(fields.len() == 3, "Malformed index metadata fields");
                let oid = fields[1];
                ensure!(
                    [40, 64].contains(&oid.len()) && oid.bytes().all(|b| b.is_ascii_hexdigit()),
                    "Malformed index object ID"
                );
                let unsupported = if fields[2] != "0" {
                    Some("Unresolved index conflict".into())
                } else if !matches!(fields[0], "100644" | "100755") {
                    Some(format!("Git mode {}", fields[0]))
                } else {
                    None
                };
                Ok((
                    Entry {
                        path: path.into(),
                        bytes: 0,
                        identity: metadata.into(),
                        unsupported,
                    },
                    oid.to_owned(),
                ))
            })
            .collect()
    })
    .await??;
    let mut result = Vec::new();
    // Bound each Git response and avoid one process per index entry.
    for chunk in entries.chunks(1024) {
        budget(deadline)?;
        let input = chunk
            .iter()
            .flat_map(|(_, oid)| oid.bytes().chain(*b"\n"))
            .collect();
        let sizes = run_git(root, &["cat-file", "--batch-check"], Some(input)).await?;
        let sizes: Vec<_> = std::str::from_utf8(&sizes)?.lines().collect();
        ensure!(
            sizes.len() == chunk.len(),
            "Incomplete index object inventory"
        );
        for ((entry, oid), size) in chunk.iter().zip(sizes) {
            let fields: Vec<_> = size.split_whitespace().collect();
            let mut unsupported = entry.unsupported.clone();
            let bytes = if fields.len() == 3 && fields[0] == oid && fields[1] == "blob" {
                fields[2].parse()?
            } else {
                unsupported = Some("Missing or non-blob index object".into());
                0
            };
            result.push(Entry {
                path: entry.path.clone(),
                bytes,
                identity: entry.identity.clone(),
                unsupported,
            });
        }
    }
    Ok(result)
}

async fn worktree(root: &Path, deadline: Instant) -> Result<Vec<Entry>> {
    let output = run_git(
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
    let root = root.to_owned();
    tokio::task::spawn_blocking(move || -> Result<Vec<_>> {
        let names = output
            .split(|byte| *byte == 0)
            .filter(|name| !name.is_empty())
            .map(|name| Ok(std::str::from_utf8(name)?.to_owned()))
            .collect::<Result<BTreeSet<_>>>()?;
        names
            .into_iter()
            .map(|name| {
                budget(deadline)?;
                let metadata = crate::paths::confined(&root, Path::new(&name)).and_then(|path| {
                    match std::fs::symlink_metadata(path) {
                        Ok(metadata) => Ok(Some(metadata)),
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
                        Err(error) => Err(error.into()),
                    }
                });
                let (bytes, identity, unsupported) = match metadata {
                    Ok(Some(metadata)) if metadata.is_file() => (
                        metadata.len(),
                        format!("{:?}", metadata.permissions()),
                        None,
                    ),
                    Ok(None) => (0, "deleted".into(), None),
                    Ok(Some(_)) => (
                        0,
                        "unsupported".into(),
                        Some("Non-regular worktree entry".into()),
                    ),
                    Err(error) => (0, "unreadable".into(), Some(error.to_string())),
                };
                Ok(Entry {
                    path: name,
                    bytes,
                    identity,
                    unsupported,
                })
            })
            .collect()
    })
    .await?
}

fn summarize(
    side: &str,
    reference: String,
    entries: Vec<Entry>,
    options: &CaptureOptions,
    protected: &[String],
    deadline: Instant,
) -> Result<SnapshotBudget> {
    let selected = options.selector()?;
    let mut protection = globset::GlobSetBuilder::new();
    for pattern in protected {
        protection.add(globset::Glob::new(pattern)?);
    }
    let protection = protection.build()?;
    let mut oversized = Vec::new();
    let mut unsupported = Vec::new();
    let mut excluded = Vec::new();
    let mut protected_exclusions = Vec::new();
    let mut total_bytes = 0;
    for entry in &entries {
        budget(deadline)?;
        if !selected(&entry.path) {
            excluded.push(entry.path.clone());
            if protection.is_match(&entry.path) {
                protected_exclusions.push(entry.path.clone());
            }
            continue;
        }
        if let Some(reason) = &entry.unsupported {
            unsupported.push(format!("{} ({reason})", entry.path));
        }
        total_bytes += entry.bytes;
        if entry.bytes > options.max_file_bytes as u64 {
            oversized.push(OversizedFile {
                path: entry.path.clone(),
                bytes: entry.bytes,
            });
        }
    }
    let maximum = oversized
        .iter()
        .map(|file| file.bytes)
        .max()
        .unwrap_or_default();
    let recommended_max_file_mib =
        (maximum > 0 && maximum <= MAX_FILE_BYTES as u64).then(|| maximum.div_ceil(1024 * 1024));
    Ok(SnapshotBudget {
        side: side.into(),
        reference,
        inventory_digest: super::digest(&serde_json::to_vec(&entries)?),
        file_count: entries.len(),
        total_bytes,
        max_files: super::MAX_FILES,
        max_bytes: options.max_bytes,
        max_file_bytes: options.max_file_bytes,
        oversized_file_count: oversized.len(),
        unsupported_entry_count: unsupported.len(),
        excluded_file_count: excluded.len(),
        protected_exclusion_count: protected_exclusions.len(),
        details_truncated: [
            oversized.len(),
            unsupported.len(),
            excluded.len(),
            protected_exclusions.len(),
        ]
        .iter()
        .any(|len| *len > DETAIL_LIMIT),
        oversized_files: oversized.into_iter().take(DETAIL_LIMIT).collect(),
        unsupported_entries: unsupported.into_iter().take(DETAIL_LIMIT).collect(),
        excluded_paths: excluded.into_iter().take(DETAIL_LIMIT).collect(),
        protected_exclusions: protected_exclusions
            .into_iter()
            .take(DETAIL_LIMIT)
            .collect(),
        recommended_max_file_mib,
    })
}

fn budget(deadline: Instant) -> Result<()> {
    ensure!(
        Instant::now() < deadline,
        "Selected metadata preflight timed out"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn aggregate_counts_survive_truncation_and_resource_failures_are_distinct() {
        let options = CaptureOptions {
            max_bytes: 100,
            max_file_bytes: 2,
            ..Default::default()
        };
        let entries = (0..105)
            .map(|index| Entry {
                path: format!("file-{index}"),
                bytes: 3,
                identity: "id".into(),
                unsupported: Some("unsupported".into()),
            })
            .collect();
        let budget = summarize(
            "base",
            "commit".into(),
            entries,
            &options,
            &[],
            Instant::now() + Duration::from_secs(10),
        )
        .unwrap();
        assert!(!budget.ready());
        assert_eq!(budget.total_bytes, 315);
        assert_eq!(budget.oversized_file_count, 105);
        assert_eq!(budget.oversized_files.len(), 100);
        assert_eq!(budget.unsupported_entry_count, 105);
        assert_eq!(budget.unsupported_entries.len(), 100);
        assert!(budget.details_truncated);
        assert_eq!(budget.recommended_max_file_mib, Some(1));
        let mut count = summarize(
            "target",
            "commit".into(),
            vec![],
            &options,
            &[],
            Instant::now() + Duration::from_secs(10),
        )
        .unwrap();
        count.file_count = super::super::MAX_FILES + 1;
        assert!(!count.ready());
        assert!(
            summarize(
                "target",
                "commit".into(),
                vec![Entry {
                    path: "file".into(),
                    bytes: 0,
                    identity: "id".into(),
                    unsupported: None
                }],
                &options,
                &[],
                Instant::now()
            )
            .is_err()
        );
    }
}
