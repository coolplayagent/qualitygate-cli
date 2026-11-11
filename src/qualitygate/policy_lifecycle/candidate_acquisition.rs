//! Acquire an immutable parent once, then archive only its policy-owned inputs.

use crate::{
    config::policy_candidates::{self, Proposal},
    snapshot,
};
use anyhow::Result;
use std::path::PathBuf;

pub async fn create(
    root: PathBuf,
    config_path: String,
    reference: String,
    proposal: Proposal,
) -> Result<serde_json::Value> {
    if reference.starts_with("sha256:") {
        return tokio::task::spawn_blocking(move || {
            policy_candidates::create_from_version(&root, &reference, proposal)
        })
        .await?;
    }
    let commit = snapshot::resolve_commit(&root, &reference).await?;
    let files = snapshot::read_commit(&root, &commit).await?;
    tokio::task::spawn_blocking(move || {
        policy_candidates::create_from_snapshot(
            &root,
            &config_path,
            &commit,
            files
                .iter()
                .map(|(name, file)| (name.as_str(), file.bytes.as_slice(), file.executable)),
            proposal,
        )
    })
    .await?
}
