//! Bind passing full checks to an artifact made from exactly the checked bytes.
use super::CheckOptions;
use crate::domain::{Decision, delivery::ExportReport};
use anyhow::{Result, ensure};
use std::path::PathBuf;

pub async fn check(
    options: CheckOptions,
    expected_base: Option<&str>,
    destination: PathBuf,
) -> Result<ExportReport> {
    ensure!(
        options.profile == "full"
            && options.snapshot_options.scope == crate::domain::check_scope::CheckScope::Delivery
            && options.snapshot_options.path_filter.is_none()
            && !matches!(options.selection, crate::snapshot::Selection::Path { .. }),
        "Delivery export requires a full unfiltered check"
    );
    let (report, snapshot) = super::check_and_snapshot(options, expected_base).await?;
    let delivery = if report.gate.complete
        && report.gate.decision == Decision::Pass
        && report.plan.pending_delivery_checks.is_empty()
        && ["delivery", "task"].contains(&report.scope.as_str())
    {
        Some(
            tokio::task::spawn_blocking(move || {
                crate::snapshot::delivery::export(&snapshot, &destination)
            })
            .await??,
        )
    } else {
        None
    };
    Ok(ExportReport {
        schema_version: 1,
        kind: "delivery_check".into(),
        report,
        delivery,
    })
}
