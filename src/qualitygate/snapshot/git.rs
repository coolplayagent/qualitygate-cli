use super::limits::Acquisition;
use super::{File, MAX_FILE_BYTES, MAX_FILES};
use anyhow::{Context, Result, bail};
use std::{collections::BTreeMap, path::Path, time::Duration};

pub async fn run_git(root: &Path, args: &[&str], input: Option<Vec<u8>>) -> Result<Vec<u8>> {
    let argv: Vec<String> = [
        "git",
        "--no-replace-objects",
        "-c",
        "core.quotepath=false",
        "-c",
        "core.fsmonitor=false",
    ]
    .iter()
    .chain(args)
    .map(|arg| (*arg).into())
    .collect();
    let output = crate::runner::capture(&argv, root, input, Duration::from_secs(30)).await?;
    if output.timed_out {
        bail!("Git operation timed out: {}", args.first().unwrap_or(&""));
    }
    if let Some(error) = output.capture_error {
        bail!(
            "Git output capture failed ({}): {error}",
            args.first().unwrap_or(&"")
        );
    }
    if output.exit_code != Some(0) {
        bail!(
            "Git {} failed: {}",
            args.first().unwrap_or(&""),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(output.stdout)
}

pub async fn resolve_commit(root: &Path, reference: &str) -> Result<String> {
    if reference.is_empty() || reference.starts_with('-') || reference.contains(['\0', '\n', '\r'])
    {
        bail!("Invalid Git reference");
    }
    let query = format!("{reference}^{{commit}}");
    let output = run_git(
        root,
        &["rev-parse", "--verify", "--end-of-options", &query],
        None,
    )
    .await?;
    let oid = std::str::from_utf8(&output)?.trim();
    if ![40, 64].contains(&oid.len()) || !oid.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        bail!("Invalid resolved commit");
    }
    Ok(oid.into())
}

pub async fn read_commit(root: &Path, reference: &str) -> Result<BTreeMap<String, File>> {
    read_commit_with_options(root, reference, &super::CaptureOptions::default()).await
}

pub async fn read_commit_with_options(
    root: &Path,
    reference: &str,
    options: &super::CaptureOptions,
) -> Result<BTreeMap<String, File>> {
    let acquisition = Acquisition::new(options)?;
    tokio::time::timeout(options.timeout, commit(root, reference, &acquisition))
        .await
        .context("Snapshot acquisition timed out")?
}

pub(super) async fn commit(
    root: &Path,
    reference: &str,
    acquisition: &Acquisition,
) -> Result<BTreeMap<String, File>> {
    let reference = resolve_commit(root, reference).await?;
    let listing = run_git(root, &["ls-tree", "-r", "-z", &reference], None).await?;
    read_entries(root, listing, false, acquisition).await
}

pub(super) async fn read_index(
    root: &Path,
    acquisition: &Acquisition,
) -> Result<BTreeMap<String, File>> {
    let listing = run_git(root, &["ls-files", "--stage", "-z"], None).await?;
    read_entries(root, listing, true, acquisition).await
}

#[derive(Debug)]
struct Entry {
    path: String,
    executable: bool,
    oid: String,
    size: usize,
}

fn entries(listing: &[u8], index: bool) -> Result<Vec<Entry>> {
    let mut entries = Vec::new();
    for entry in listing
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
    {
        let entry = std::str::from_utf8(entry)?;
        let (metadata, path) = entry.split_once('\t').context("Malformed Git entry")?;
        let fields: Vec<&str> = metadata.split_whitespace().collect();
        if fields.len() != 3 {
            bail!("Malformed Git entry metadata");
        }
        if !["100644", "100755"].contains(&fields[0]) {
            bail!("Unsupported Git mode {} at {path}", fields[0]);
        }
        if index && fields[2] != "0" {
            bail!("Unresolved index conflict: {path}");
        }
        let oid = fields[if index { 1 } else { 2 }];
        if ![40, 64].contains(&oid.len()) || !oid.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            bail!("Invalid Git object identity at {path}");
        }
        let path = crate::paths::relative(Path::new(path))?;
        entries.push(Entry {
            path,
            executable: fields[0] == "100755",
            oid: oid.into(),
            size: 0,
        });
    }
    if entries.len() > MAX_FILES {
        bail!("Snapshot exceeds {MAX_FILES} files");
    }
    Ok(entries)
}

fn requests(entries: &[Entry]) -> Vec<u8> {
    entries
        .iter()
        .flat_map(|entry| entry.oid.bytes().chain(*b"\n"))
        .collect()
}

fn header<'a>(bytes: &'a [u8], entry: &Entry) -> Result<(&'a [u8], usize)> {
    let newline = bytes
        .iter()
        .position(|byte| *byte == b'\n')
        .context("Missing Git blob header")?;
    let fields: Vec<_> = std::str::from_utf8(&bytes[..newline])?
        .split_whitespace()
        .collect();
    if fields.len() != 3 || fields[1] != "blob" {
        bail!("Git object is not a blob: {}", entry.path);
    }
    if fields[0] != entry.oid {
        bail!("Git blob identity mismatch: {}", entry.path);
    }
    Ok((&bytes[newline + 1..], fields[2].parse()?))
}

// At most 4 MiB of content plus 1024 bounded headers per process, below the
// runner's unchanged 16 MiB stream limit. Inspect sizes before requesting bytes.
const BATCH_BYTES: usize = 4 * 1024 * 1024;
const BATCH_FILES: usize = 1024;

fn batches(mut entries: Vec<Entry>, sizes: &[u8], budget: usize) -> Result<Vec<Vec<Entry>>> {
    let mut cursor = sizes;
    let mut total = 0;
    for entry in &mut entries {
        let (rest, size) = header(cursor, entry)?;
        if size > MAX_FILE_BYTES {
            bail!("File exceeds {MAX_FILE_BYTES} bytes: {}", entry.path);
        }
        total += size;
        if total > budget {
            bail!(
                "Snapshot exceeds {budget} bytes at {}; adjust --snapshot-max-mib",
                entry.path
            );
        }
        entry.size = size;
        cursor = rest;
    }
    if !cursor.is_empty() {
        bail!("Unexpected trailing Git object data");
    }
    let mut batches = Vec::new();
    let mut batch = Vec::new();
    let mut bytes = 0;
    for entry in entries {
        if batch.len() == BATCH_FILES || bytes + entry.size > BATCH_BYTES {
            batches.push(std::mem::take(&mut batch));
            bytes = 0;
        }
        bytes += entry.size;
        batch.push(entry);
    }
    if !batch.is_empty() {
        batches.push(batch);
    }
    Ok(batches)
}

fn blobs(entries: Vec<Entry>, output: &[u8]) -> Result<BTreeMap<String, File>> {
    let mut cursor = output;
    let mut files = BTreeMap::new();
    for entry in entries {
        let (rest, size) = header(cursor, &entry)?;
        if size != entry.size || rest.len() <= size || rest[size] != b'\n' {
            bail!("Truncated or changed Git blob: {}", entry.path);
        }
        files.insert(
            entry.path,
            File {
                bytes: rest[..size].to_vec(),
                executable: entry.executable,
            },
        );
        cursor = &rest[size + 1..];
    }
    if !cursor.is_empty() {
        bail!("Unexpected trailing Git object data");
    }
    Ok(files)
}

async fn read_entries(
    root: &Path,
    listing: Vec<u8>,
    index: bool,
    acquisition: &Acquisition,
) -> Result<BTreeMap<String, File>> {
    let entries = tokio::task::spawn_blocking(move || entries(&listing, index)).await??;
    if entries.is_empty() {
        return Ok(BTreeMap::new());
    }
    let input = requests(&entries);
    let sizes = {
        let _permit = acquisition.permits.acquire().await?;
        run_git(root, &["cat-file", "--batch-check"], Some(input)).await?
    };
    let budget = acquisition.options.max_bytes;
    let batches = tokio::task::spawn_blocking(move || batches(entries, &sizes, budget)).await??;
    let mut pending = batches.into_iter();
    let mut tasks = tokio::task::JoinSet::new();
    let mut files = BTreeMap::new();
    loop {
        while tasks.len() < acquisition.options.jobs {
            let Some(batch) = pending.next() else { break };
            let root = root.to_owned();
            let permits = acquisition.permits.clone();
            tasks.spawn(async move {
                let _permit = permits.acquire_owned().await?;
                let output =
                    run_git(&root, &["cat-file", "--batch"], Some(requests(&batch))).await?;
                tokio::task::spawn_blocking(move || blobs(batch, &output)).await?
            });
        }
        let Some(result) = tasks.join_next().await else {
            break;
        };
        files.extend(result??);
    }
    Ok(files)
}

pub(super) async fn messages(root: &Path, base: &str, head: &str) -> Result<Vec<(String, String)>> {
    let range = format!("{base}..{head}");
    let listing = run_git(root, &["rev-list", "--max-count=1001", &range], None).await?;
    let listing = std::str::from_utf8(&listing)?;
    if listing.lines().count() > 1000 {
        bail!("Commit range exceeds 1000 commits");
    }
    let mut messages = Vec::new();
    for oid in listing.lines() {
        let message = run_git(root, &["show", "-s", "--format=%B", oid], None).await?;
        messages.push((oid.into(), String::from_utf8(message)?));
    }
    Ok(messages)
}

#[cfg(test)]
#[path = "git_tests.rs"]
mod tests;
