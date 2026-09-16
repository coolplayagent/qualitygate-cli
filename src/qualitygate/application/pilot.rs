//! Filesystem loading and CPU work are isolated from asynchronous orchestration.
use anyhow::Result;
use serde_json::Value;
use std::path::PathBuf;

pub async fn summarize(input: PathBuf) -> Result<(Value, u8)> {
    tokio::task::spawn_blocking(move || {
        let (manifest, reports) = crate::config::pilot::load(&input)?;
        let now = crate::config::policy_store::now()?;
        let summary = crate::domain::pilot::summarize(&manifest, &reports, now)?;
        let code = if summary["complete"] == true { 0 } else { 2 };
        Ok((summary, code))
    })
    .await?
}
