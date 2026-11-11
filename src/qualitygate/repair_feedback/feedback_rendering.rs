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
    tokio::task::spawn_blocking(move || render_report(&report, max_bytes, severity)).await?
}

/// Render feedback while returning the same report for subsequent evidence use.
pub async fn render_retaining(
    report: Report,
    max_bytes: usize,
    severity: Option<Severity>,
) -> Result<(Report, String)> {
    tokio::task::spawn_blocking(move || {
        let feedback = render_report(&report, max_bytes, severity)?;
        Ok((report, feedback))
    })
    .await?
}

fn render_report(report: &Report, max_bytes: usize, severity: Option<Severity>) -> Result<String> {
    let path = report
        .context
        .as_ref()
        .context("Feedback requires a persisted report context")?
        .report_path
        .clone();
    let bytes = serde_json::to_vec_pretty(report)?;
    let artifact = crate::domain::Artifact {
        path,
        digest: snapshot::digest(&bytes),
        bytes: bytes.len() as u64,
    };
    crate::domain::feedback::render(report, artifact, max_bytes, severity)
}

#[cfg(test)]
mod tests {
    #[tokio::test(flavor = "current_thread")]
    async fn retaining_feedback_preserves_report_and_exact_artifact_identity() {
        let report: crate::domain::Report = serde_json::from_value(serde_json::json!({
            "schema_version":1,"run_id":"feedback-test","scope":"task","profile":"full",
            "snapshot":{"mode":"worktree","base":"base","head":"head","content_digest":"snapshot"},
            "policy":{"source":"policy","trust":"caller_supplied_ref","config_digest":"config",
                "rules_digest":"rules","task_contract_digest":"task","changes":[]},
            "plan":{"required_checks":[],"pending_delivery_checks":[],"acceptance":{}},
            "gate":{"complete":true,"decision":"pass","blockers":[]},
            "checks":[],"summary":crate::domain::Summary::default(),
            "context":{"report_path":"/evidence/report.json","recheck":{"argv":["qualitygate","check"]},
                "delivery_recheck":{"argv":["qualitygate","check","--profile","full"]}}
        })).unwrap();
        let bytes = serde_json::to_vec_pretty(&report).unwrap();
        let expected = super::render(report.clone(), 32768, None).await.unwrap();
        let (report, feedback) = super::render_retaining(report, 32768, None).await.unwrap();
        assert_eq!(feedback, expected);
        assert_eq!(serde_json::to_vec_pretty(&report).unwrap(), bytes);
        let value: serde_json::Value = serde_json::from_str(&feedback).unwrap();
        assert_eq!(value["full_report"]["bytes"], bytes.len());
        assert_eq!(
            value["full_report"]["digest"],
            crate::snapshot::digest(&bytes)
        );
        assert_eq!(value["full_report"]["path"], "/evidence/report.json");
    }
}
