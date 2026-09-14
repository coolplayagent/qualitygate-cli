//! Progressive context resolved from the caller's immutable policy reference.

use crate::{config, snapshot};
use anyhow::Result;
use serde_json::Value;
use std::path::PathBuf;

pub async fn read(
    root: PathBuf,
    path: String,
    category: Option<String>,
    policy_ref: Option<String>,
) -> Result<Value> {
    if let Some(active) = super::policy_active::load(root.clone()).await? {
        if policy_ref
            .as_ref()
            .is_some_and(|reference| reference != &active.active.reference)
        {
            anyhow::bail!(
                "Context must include the selected active policy; historical replay uses candidate validate"
            );
        }
        return tokio::task::spawn_blocking(move || {
            let mut result = config::rule_query::context(&root, &path, category.as_deref())?;
            result["review_trust"] = serde_json::json!("signed_active_policy");
            Ok(result)
        })
        .await?;
    }
    if let Some(reference) = policy_ref {
        let commit = snapshot::resolve_commit(&root, &reference).await?;
        let files = snapshot::read_commit(&root, &commit).await?;
        tokio::task::spawn_blocking(move || {
            let mut result = config::rule_query::context_from_files(
                &path,
                files
                    .iter()
                    .map(|(name, file)| (name.as_str(), file.bytes.as_slice())),
                category.as_deref(),
            )?;
            result["review_trust"] = serde_json::json!("caller_supplied_ref");
            result["policy_ref"] = serde_json::json!(commit);
            Ok(result)
        })
        .await?
    } else {
        tokio::task::spawn_blocking(move || {
            config::rule_query::context(&root, &path, category.as_deref())
        })
        .await?
    }
}
