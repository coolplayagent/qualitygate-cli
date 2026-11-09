//! Capture immutable commit parents, trees and input-only parsed trailers.

use super::{
    File, Snapshot, content_digest_until, digest, git::run_git_before, read_commit_with_options,
};
use anyhow::{Context, Result, bail};
use std::{
    collections::BTreeMap,
    sync::Arc,
    time::{Duration, Instant},
};

pub const MAX_COMMITS: usize = 1000;
pub const MAX_HISTORY_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_TREE_ENTRIES: usize = 200_000;

#[derive(Debug)]
pub struct Commit {
    /// Object identity captured from the immutable Git input.
    pub oid: String,
    /// Ordered parent object identities.
    pub parents: Vec<String>,
    /// Interned tree contents keyed by repository-relative path.
    pub files: BTreeMap<String, Arc<File>>,
    /// Digest of the tree contents used to validate cross-domain consumers.
    pub content_digest: String,
    /// Commit message captured without executing configured Git hooks.
    pub message: String,
    /// Normalized trailer values keyed by lower-case trailer name.
    pub trailers: BTreeMap<String, Vec<String>>,
}

#[derive(Debug)]
pub struct History {
    /// Ordered, bounded commits consumed by declaration adapters.
    ///
    /// The declaration analyzer treats this as untrusted input and validates
    /// topology, content digests, and comparison identities.
    pub commits: Vec<Commit>,
}

type BlobPool = BTreeMap<(String, bool), Arc<File>>;

struct Interned {
    pool: BlobPool,
    files: BTreeMap<String, Arc<File>>,
    content_digest: String,
    added_bytes: usize,
}

fn intern(
    source: BTreeMap<String, File>,
    mut pool: BlobPool,
    deadline: Instant,
    byte_budget: usize,
) -> Result<Interned> {
    let content_digest = content_digest_until(&source, Some(deadline))?;
    let mut added_bytes = 0;
    let mut files = BTreeMap::new();
    for (path, file) in source {
        remaining(deadline)?;
        let key = (digest(&file.bytes), file.executable);
        let file = match pool.entry(key) {
            std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::btree_map::Entry::Vacant(entry) => {
                added_bytes += file.bytes.len();
                if added_bytes > byte_budget {
                    bail!("Git declaration history exceeds 64 MiB of unique contents");
                }
                entry.insert(Arc::new(file))
            }
        };
        files.insert(path, Arc::clone(file));
    }
    remaining(deadline)?;
    Ok(Interned {
        pool,
        files,
        content_digest,
        added_bytes,
    })
}

pub async fn capture(snapshot: &Snapshot) -> Result<History> {
    capture_with_options(
        snapshot,
        &super::CaptureOptions {
            exclude: snapshot.scope_evidence.exclude.clone(),
            unstructured_documents: snapshot.scope_evidence.resource_exclusions.policy.clone(),
            protected_paths: snapshot
                .scope_evidence
                .resource_exclusions
                .protected_paths
                .clone(),
            ..Default::default()
        },
    )
    .await
}

pub async fn capture_with_options(
    snapshot: &Snapshot,
    options: &super::CaptureOptions,
) -> Result<History> {
    capture_before(snapshot, options, Instant::now() + Duration::from_secs(60)).await
}

fn remaining(deadline: Instant) -> Result<Duration> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        bail!("Git declaration history exceeded its 60-second acquisition budget");
    }
    Ok(remaining)
}

async fn capture_before(
    snapshot: &Snapshot,
    options: &super::CaptureOptions,
    deadline: Instant,
) -> Result<History> {
    before_deadline(deadline, capture_inner(snapshot, options, deadline)).await
}

async fn before_deadline<T>(
    deadline: Instant,
    future: impl std::future::Future<Output = Result<T>>,
) -> Result<T> {
    let result = tokio::time::timeout(remaining(deadline)?, future)
        .await
        .context("Git declaration history exceeded its 60-second acquisition budget")?;
    remaining(deadline)?;
    result
}

async fn capture_inner(
    snapshot: &Snapshot,
    options: &super::CaptureOptions,
    deadline: Instant,
) -> Result<History> {
    let root = &snapshot.root;
    let listing = run_git_before(
        root,
        &[
            "rev-list",
            "--topo-order",
            "--reverse",
            "--max-count=1001",
            &snapshot.identity.base,
            &snapshot.identity.head,
            "--",
        ],
        None,
        deadline,
    )
    .await?;
    let oids: Vec<_> = std::str::from_utf8(&listing)?.lines().collect();
    if oids.len() > MAX_COMMITS {
        bail!("Git declaration history exceeds {MAX_COMMITS} commits");
    }
    let mut blobs = BlobPool::new();
    let mut bytes = 0;
    let mut entries = 0;
    let mut commits = Vec::new();
    for oid in oids {
        remaining(deadline)?;
        let raw = run_git_before(root, &["cat-file", "commit", oid], None, deadline).await?;
        charge(&mut bytes, raw.len())?;
        let (parents, message, input) =
            tokio::task::spawn_blocking(move || decode_commit(raw, deadline)).await??;
        // --parse never adds configured trailers or invokes configured producers.
        let parsed = run_git_before(
            root,
            &[
                "-c",
                "trailer.separators=:",
                "interpret-trailers",
                "--parse",
                "--no-divider",
            ],
            Some(input),
            deadline,
        )
        .await?;
        charge(&mut bytes, parsed.len())?;
        let trailers =
            tokio::task::spawn_blocking(move || decode_trailers(&parsed, deadline)).await??;
        let mut bounded = options.clone();
        bounded.timeout = bounded.timeout.min(remaining(deadline)?);
        let source = read_commit_with_options(root, oid, &bounded).await?;
        remaining(deadline)?;
        entries += source.len();
        if entries > MAX_TREE_ENTRIES {
            bail!("Git declaration history exceeds 200000 tree entries");
        }
        let interned = tokio::task::spawn_blocking(move || {
            intern(source, blobs, deadline, MAX_HISTORY_BYTES - bytes)
        })
        .await??;
        blobs = interned.pool;
        bytes += interned.added_bytes;
        remaining(deadline)?;
        commits.push(Commit {
            oid: oid.into(),
            parents,
            files: interned.files,
            content_digest: interned.content_digest,
            message,
            trailers,
        });
    }
    remaining(deadline)?;
    Ok(History { commits })
}

fn charge(bytes: &mut usize, added: usize) -> Result<()> {
    *bytes = bytes.saturating_add(added);
    if *bytes > MAX_HISTORY_BYTES {
        bail!("Git declaration history exceeds 64 MiB of unique contents");
    }
    Ok(())
}

type CommitInput = (Vec<String>, String, Vec<u8>);

fn decode_commit(mut raw: Vec<u8>, deadline: Instant) -> Result<CommitInput> {
    remaining(deadline)?;
    let (headers, _) = std::str::from_utf8(&raw)?
        .split_once("\n\n")
        .context("Malformed Git commit object")?;
    let message_start = headers.len() + 2;
    let mut parents = Vec::new();
    for line in headers.lines() {
        remaining(deadline)?;
        if let Some(parent) = line.strip_prefix("parent ") {
            parents.push(parent.to_owned());
        }
    }
    // Reuse the raw allocation for the retained message. Only the subprocess
    // stdin needs a second copy, and that copy is also made off the async worker.
    raw.drain(..message_start);
    let message = String::from_utf8(raw)?;
    let input = message.as_bytes().to_vec();
    remaining(deadline)?;
    Ok((parents, message, input))
}

fn decode_trailers(parsed: &[u8], deadline: Instant) -> Result<BTreeMap<String, Vec<String>>> {
    remaining(deadline)?;
    let mut trailers: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for line in std::str::from_utf8(parsed)?.lines() {
        remaining(deadline)?;
        let (name, value) = line
            .split_once(':')
            .context("Malformed parsed Git trailer")?;
        trailers
            .entry(name.trim().to_ascii_lowercase())
            .or_default()
            .push(value.trim().into());
    }
    remaining(deadline)?;
    Ok(trailers)
}

#[cfg(test)]
#[path = "history_tests.rs"]
mod tests;
