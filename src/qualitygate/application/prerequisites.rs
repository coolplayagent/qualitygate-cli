use crate::domain::prerequisites::{FailureCode, Phase, PrerequisiteIssue};
use anyhow::Result;
use std::path::PathBuf;

pub async fn root(path: PathBuf) -> Result<PathBuf> {
    tokio::task::spawn_blocking(move || {
        let issue = || {
            PrerequisiteIssue::new(
                FailureCode::RootUnavailable,
                Phase::Entry,
                "Repository root must be an accessible directory",
            )
            .resource(path.display().to_string())
            .instruction("Select an existing directory with --root.")
        };
        let root = path
            .canonicalize()
            .map_err(|error| issue().wrap(error.into()))?;
        if !std::fs::metadata(&root)
            .map_err(|error| issue().wrap(error.into()))?
            .is_dir()
        {
            return Err(issue().into());
        }
        Ok(root)
    })
    .await?
}

pub(crate) fn record(result: &mut crate::domain::CheckResult, error: &anyhow::Error) {
    let mut issue = PrerequisiteIssue::from_error(error);
    issue.check_id = Some(result.id.clone());
    result
        .metadata
        .entry("prerequisites".into())
        .or_insert_with(|| serde_json::json!([]))
        .as_array_mut()
        .expect("prerequisites is an array")
        .push(serde_json::json!(issue));
}

pub(crate) fn identity_changed(message: impl Into<String>) -> anyhow::Error {
    PrerequisiteIssue::new(FailureCode::ToolIdentityChanged, Phase::Execution, message)
        .instruction("Restore a stable tool installation and rerun; evidence from changed executables cannot be accepted.")
        .into()
}

pub(crate) fn record_evidence(result: &mut crate::domain::CheckResult, error: anyhow::Error) {
    let error = PrerequisiteIssue::new(
        FailureCode::EvidenceInvalid,
        Phase::Evidence,
        "Execution evidence could not be validated",
    )
    .instruction("Repair the producer or evidence inputs and rerun; do not reuse stale or mismatched reports.")
    .wrap(error);
    record(result, &error);
}

pub(crate) async fn evidence_directory(base: Option<PathBuf>) -> Result<PathBuf> {
    tokio::task::spawn_blocking(move || crate::paths::run_directory(base.as_deref())).await?
        .map_err(|error| PrerequisiteIssue::new(FailureCode::StorageUnavailable, Phase::Prepare,
            "Cannot create the evidence directory")
            .instruction("Check storage permissions/capacity or choose a writable --output-dir or QUALITYGATE_HOME.").wrap(error))
}
