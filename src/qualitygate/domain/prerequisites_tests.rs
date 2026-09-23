use super::*;

#[test]
fn typed_errors_survive_context_and_keep_the_original_cause() {
    let issue = PrerequisiteIssue::new(FailureCode::InputMissing, Phase::Inputs, "Task is missing")
        .resource("task.yaml")
        .instruction("Restore the selected task contract.");
    let error = issue
        .wrap(anyhow::anyhow!("file absent"))
        .context("Check preparation failed");
    let error = PrerequisiteIssue::new(FailureCode::PlanInvalid, Phase::Prepare, "Plan invalid")
        .wrap(error);
    let observed = PrerequisiteIssue::from_error(&error);
    assert_eq!(observed.code, FailureCode::InputMissing);
    let cause = observed.cause.as_deref().unwrap();
    assert!(cause.contains("file absent"));
    assert!(cause.contains("Check preparation failed"));
    assert_eq!(observed.resource.as_deref(), Some("task.yaml"));
    let report = CommandError::new(observed);
    assert_eq!(report.gate.decision, Decision::Incomplete);
    assert!(!report.gate.complete);
    assert!(report.verification.verified_shapes.is_empty());
    let decoded: CommandError =
        serde_json::from_value(serde_json::to_value(report).unwrap()).unwrap();
    assert_eq!(decoded.kind, "command_error");
}

#[test]
fn unknown_errors_do_not_invent_initialization_or_execution_evidence() {
    let issue = PrerequisiteIssue::from_error(&anyhow::anyhow!("run qualitygate init first"));
    assert_eq!(issue.code, FailureCode::UnknownError);
    assert!(matches!(
        issue.next_actions[0],
        RecoveryAction::Instruction { .. }
    ));
    let value = serde_json::to_value(CommandError::new(issue)).unwrap();
    assert!(value.get("run_id").is_none());
    assert!(value.get("snapshot").is_none());
    let error = anyhow::anyhow!("original failure").context("outer context");
    let issue = PrerequisiteIssue::from_error(&error);
    assert_eq!(
        issue.cause.as_deref(),
        Some("outer context: original failure")
    );
}
