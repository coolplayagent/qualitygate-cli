use super::{resolve_commit, run_git};
use crate::{domain::MergeRequest, net::merge_request::Request};
use anyhow::{Result, bail};
use std::path::Path;

pub(super) async fn resolve(
    root: &Path,
    url: &str,
    api_base: Option<&str>,
) -> Result<MergeRequest> {
    let request = Request::parse(url, api_base)?;
    let origin = run_git(root, &["remote", "get-url", "origin"], None).await?;
    if !request.matches_remote(std::str::from_utf8(&origin)?.trim())? {
        bail!("MR target repository does not match this checkout's origin");
    }
    let mut comparison = request.resolve().await?;
    ensure_objects(root, &comparison.target_head, &comparison.source_head).await?;
    if run_git(root, &["rev-parse", "--is-shallow-repository"], None).await? == b"true\n" {
        bail!("MR comparison requires complete Git ancestry; fetch full history before checking");
    }
    let bases = run_git(
        root,
        &[
            "merge-base",
            "--all",
            &comparison.target_head,
            &comparison.source_head,
        ],
        None,
    )
    .await?;
    let bases = std::str::from_utf8(&bases)?.lines().collect::<Vec<_>>();
    if bases.len() != 1 {
        bail!(
            "MR comparison needs one unambiguous merge base; found {}",
            bases.len()
        );
    }
    comparison.merge_base = resolve_commit(root, bases[0]).await?;
    Ok(comparison)
}

async fn ensure_objects(root: &Path, target: &str, source: &str) -> Result<()> {
    if resolve_commit(root, target).await.is_err() || resolve_commit(root, source).await.is_err() {
        // Fetch immutable object IDs without moving local branches, remote-tracking
        // refs, tags, FETCH_HEAD, the index or the user's working-tree files.
        run_git(
            root,
            &[
                "fetch",
                "--no-tags",
                "--no-recurse-submodules",
                "--no-write-fetch-head",
                "origin",
                target,
                source,
            ],
            None,
        )
        .await?;
    }
    if resolve_commit(root, target).await? != target
        || resolve_commit(root, source).await? != source
    {
        bail!("Fetched objects do not match provider commit identities");
    }
    Ok(())
}
