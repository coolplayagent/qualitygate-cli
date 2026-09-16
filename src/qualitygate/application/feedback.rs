//! Present already-persisted, unfiltered reports without changing gate decisions.
use crate::{
    domain::{Report, Severity},
    snapshot,
};
use anyhow::{Context, Result};

pub async fn render(
    report: Report,
    max_bytes: usize,
    severity: Option<Severity>,
) -> Result<String> {
    tokio::task::spawn_blocking(move || {
        let path = report
            .context
            .as_ref()
            .context("Feedback requires a persisted report context")?
            .report_path
            .clone();
        let bytes = serde_json::to_vec_pretty(&report)?;
        let artifact = crate::domain::Artifact {
            path,
            digest: snapshot::digest(&bytes),
            bytes: bytes.len() as u64,
        };
        crate::domain::feedback::render(&report, artifact, max_bytes, severity)
    })
    .await?
}
