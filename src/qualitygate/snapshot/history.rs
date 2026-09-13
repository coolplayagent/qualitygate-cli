//! Capture immutable commit parents, trees and input-only parsed trailers.

use super::{File, Snapshot, content_digest, digest, read_commit, run_git};
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

fn intern(source: BTreeMap<String, File>, mut pool: BlobPool) -> Interned {
    let content_digest = content_digest(&source);
    let mut added_bytes = 0;
    let mut files = BTreeMap::new();
    for (path, file) in source {
        let key = (digest(&file.bytes), file.executable);
        let file = pool.entry(key).or_insert_with(|| {
            added_bytes += file.bytes.len();
            Arc::new(file)
        });
        files.insert(path, Arc::clone(file));
    }
    Interned {
        pool,
        files,
        content_digest,
        added_bytes,
    }
}

pub async fn capture(snapshot: &Snapshot) -> Result<History> {
    let started = Instant::now();
    let root = &snapshot.root;
    let listing = run_git(
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
        if started.elapsed() > Duration::from_secs(60) {
            bail!("Git declaration history exceeded its 60-second acquisition budget");
        }
        let raw = run_git(root, &["cat-file", "commit", oid], None).await?;
        let (headers, message) = std::str::from_utf8(&raw)?
            .split_once("\n\n")
            .context("Malformed Git commit object")?;
        let parents = headers
            .lines()
            .filter_map(|line| line.strip_prefix("parent "))
            .map(str::to_owned)
            .collect();
        // --parse never adds configured trailers or invokes configured producers.
        let parsed = run_git(
            root,
            &[
                "-c",
                "trailer.separators=:",
                "interpret-trailers",
                "--parse",
                "--no-divider",
            ],
            Some(message.as_bytes().to_vec()),
        )
        .await?;
        let mut trailers: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for line in std::str::from_utf8(&parsed)?.lines() {
            let (name, value) = line
                .split_once(':')
                .context("Malformed parsed Git trailer")?;
            trailers
                .entry(name.trim().to_ascii_lowercase())
                .or_default()
                .push(value.trim().into());
        }
        let source = read_commit(root, oid).await?;
        entries += source.len();
        bytes += raw.len() + parsed.len();
        let interned = tokio::task::spawn_blocking(move || intern(source, blobs)).await?;
        blobs = interned.pool;
        bytes += interned.added_bytes;
        if bytes > MAX_HISTORY_BYTES || entries > MAX_TREE_ENTRIES {
            bail!(
                "Git declaration history exceeds 64 MiB of unique contents or 200000 tree entries"
            );
        }
        commits.push(Commit {
            oid: oid.into(),
            parents,
            files: interned.files,
            content_digest: interned.content_digest,
            message: message.into(),
            trailers,
        });
    }
    Ok(History { commits })
}
