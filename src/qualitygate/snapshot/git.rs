use super::{File, MAX_FILE_BYTES, MAX_FILES, MAX_SNAPSHOT_BYTES};
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
        bail!("Git output capture failed: {error}");
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
    let reference = resolve_commit(root, reference).await?;
    let listing = run_git(root, &["ls-tree", "-r", "-z", &reference], None).await?;
    read_entries(root, &listing, false).await
}

pub(super) async fn read_index(root: &Path) -> Result<BTreeMap<String, File>> {
    let listing = run_git(root, &["ls-files", "--stage", "-z"], None).await?;
    read_entries(root, &listing, true).await
}

async fn read_entries(root: &Path, listing: &[u8], index: bool) -> Result<BTreeMap<String, File>> {
    let mut entries = Vec::new();
    let mut input = Vec::new();
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
        let path = crate::paths::relative(Path::new(path))?;
        entries.push((path, fields[0] == "100755"));
        input.extend_from_slice(oid.as_bytes());
        input.push(b'\n');
    }
    if entries.len() > MAX_FILES {
        bail!("Snapshot exceeds {MAX_FILES} files");
    }
    if entries.is_empty() {
        return Ok(BTreeMap::new());
    }
    let output = run_git(root, &["cat-file", "--batch"], Some(input)).await?;
    let mut cursor = output.as_slice();
    let mut total = 0;
    let mut files = BTreeMap::new();
    for (path, executable) in entries {
        let newline = cursor
            .iter()
            .position(|byte| *byte == b'\n')
            .context("Missing Git blob header")?;
        let header: Vec<&str> = std::str::from_utf8(&cursor[..newline])?
            .split_whitespace()
            .collect();
        if header.len() != 3 || header[1] != "blob" {
            bail!("Git object is not a blob: {path}");
        }
        let size: usize = header[2].parse()?;
        total += size;
        if size > MAX_FILE_BYTES || total > MAX_SNAPSHOT_BYTES {
            bail!("Snapshot input budget exceeded at {path}");
        }
        cursor = &cursor[newline + 1..];
        if cursor.len() <= size || cursor[size] != b'\n' {
            bail!("Truncated Git blob: {path}");
        }
        files.insert(
            path,
            File {
                bytes: cursor[..size].to_vec(),
                executable,
            },
        );
        cursor = &cursor[size + 1..];
    }
    if !cursor.is_empty() {
        bail!("Unexpected trailing Git object data");
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
