//! Bound evidence filenames to check identity on every supported filesystem.

use crate::{domain::Artifact, snapshot};
use anyhow::Result;
use std::path::Path;

pub(super) async fn persist(
    root: &Path,
    check: &str,
    part: &str,
    bytes: &[u8],
) -> Result<Artifact> {
    let identity = snapshot::digest(check.as_bytes());
    let path = root.join(format!(
        "check-{}-{part}",
        identity.trim_start_matches("sha256:")
    ));
    tokio::fs::write(&path, bytes).await?;
    Ok(Artifact {
        path: path.display().to_string(),
        digest: snapshot::digest(bytes),
        bytes: bytes.len() as u64,
    })
}
