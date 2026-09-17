use super::*;
use std::collections::BTreeSet;

fn scheduled_plan() -> Manifest {
    let mut manifest = sourced_plan();
    manifest.schema_version = 5;
    manifest.run_order = manifest.assignments.iter().map(|a| a.id.clone()).collect();
    manifest
}

#[test]
fn v5_seals_alternating_order_and_audits_observed_start_sequence() {
    let plan = scheduled_plan();
    let sealed = seal(plan.clone(), 1).unwrap();
    let summary = summarize(&sealed, &Reports::new(), 1).unwrap();
    assert_eq!(summary["schedule_audit"]["status"], "incomplete");
    assert_eq!(
        summary["schedule_audit"]["planned"]
            .as_array()
            .unwrap()
            .len(),
        8
    );

    let mut observed = sealed.clone();
    observed.observations.push(Observation {
        assignment_id: observed.run_order[0].clone(),
        observed_at: 2,
        start_sequence: Some(1),
        model_evidence: None,
        attempts: Vec::new(),
        findings: Vec::new(),
        review_active_ms: None,
        review_comments: None,
        rework_rounds: None,
    });
    let partial = summarize(&observed, &Reports::new(), 2).unwrap();
    assert_eq!(
        partial["schedule_audit"]["observed"][0]["start_sequence"],
        1
    );
    assert_eq!(partial["schedule_audit"]["status"], "incomplete");

    observed.observations[0].start_sequence = Some(2);
    let deviated = summarize(&observed, &Reports::new(), 2).unwrap();
    assert_eq!(deviated["schedule_audit"]["status"], "deviated");
    assert_eq!(deviated["protocol_ready"], false);
    assert_eq!(
        deviated["schedule_audit"]["deviations"][0]["planned_position"],
        1
    );

    let mut repeated = observed.clone();
    repeated.observations.push(Observation {
        assignment_id: repeated.run_order[1].clone(),
        start_sequence: Some(2),
        model_evidence: None,
        ..repeated.observations[0].clone()
    });
    assert!(
        validate(&repeated)
            .unwrap_err()
            .to_string()
            .contains("repeated")
    );
    observed.observations[0].start_sequence = Some(0);
    assert!(validate(&observed).is_err());
    observed.observations[0].start_sequence = Some(9);
    assert!(validate(&observed).is_err());

    let mut missing = plan.clone();
    missing.run_order.pop();
    assert!(validate(&missing).is_err());
    let mut duplicate = plan.clone();
    duplicate.run_order[1] = duplicate.run_order[0].clone();
    assert!(validate(&duplicate).is_err());
    let mut unknown = plan.clone();
    unknown.run_order[0] = "unknown".into();
    assert!(validate(&unknown).is_err());
    let mut no_alternation = plan.clone();
    no_alternation.run_order.swap(1, 2);
    assert!(validate(&no_alternation).is_err());
    let mut drifted = sealed;
    drifted.run_order.swap(0, 2);
    assert!(validate(&drifted).unwrap_err().to_string().contains("seal"));

    let mut legacy = plan;
    legacy.schema_version = 4;
    assert!(validate(&legacy).is_err());
    legacy.run_order.clear();
    legacy.observations = observed.observations;
    assert!(validate(&legacy).is_err());
}

#[test]
fn v5_rejects_unbalanced_workflow_first_counts_within_a_stratum() {
    let mut plan = scheduled_plan();
    plan.protocol.task_count = 3;
    *plan
        .protocol
        .task_mix
        .as_mut()
        .unwrap()
        .get_mut(&TaskKind::BugFix)
        .unwrap() = 2;
    let new_assignments: Vec<_> = plan.assignments[..4]
        .iter()
        .cloned()
        .map(|mut assignment| {
            assignment.id.push_str("-second-bug");
            assignment.input_id = "input-second-bug".into();
            assignment.task_id = "second-bug-task".into();
            assignment.initial_snapshot = d(900);
            assignment.task_digest = d(901);
            assignment
        })
        .collect();
    plan.run_order
        .extend(new_assignments.iter().map(|a| a.id.clone()));
    plan.assignments.extend(new_assignments);
    plan.sources.push(TaskSource {
        input_id: "input-second-bug".into(),
        kind: SourceKind::Issue,
        source_id: "fixture/issues/3".into(),
        path: "second-bug-source.json".into(),
        digest: d(902),
        bytes: 32,
        selected_at: 1,
    });
    assert!(
        validate(&plan)
            .unwrap_err()
            .to_string()
            .contains("counterbalanced")
    );
}

#[test]
fn v5_seals_eight_inputs_with_two_first_positions_per_stratum() {
    let mut plan = scheduled_plan();
    plan.protocol.task_count = 8;
    plan.protocol.task_mix = Some(
        [(TaskKind::BugFix, 4), (TaskKind::Refactor, 4)]
            .into_iter()
            .collect(),
    );
    for kind_index in 0..2 {
        let base = plan.assignments[kind_index * 4..kind_index * 4 + 4].to_vec();
        let source = plan.sources[kind_index].clone();
        for replica in 1..4 {
            let input_id = format!("input-{kind_index}-{replica}");
            for mut assignment in base.iter().cloned() {
                assignment.id = format!("{}-{replica}", assignment.id);
                assignment.input_id = input_id.clone();
                assignment.task_id = input_id.clone();
                assignment.task_digest = d(1000 + kind_index * 10 + replica);
                assignment.initial_snapshot = d(1100 + kind_index * 10 + replica);
                plan.assignments.push(assignment);
            }
            let mut source = source.clone();
            source.input_id = input_id.clone();
            source.source_id = format!("fixture/issues/{kind_index}-{replica}");
            source.path = format!("source-{kind_index}-{replica}.json");
            source.digest = d(1200 + kind_index * 10 + replica);
            plan.sources.push(source);
        }
    }
    plan.run_order.clear();
    for kind in [TaskKind::BugFix, TaskKind::Refactor] {
        for model in ["medium-model", "lower-model"] {
            let inputs: Vec<_> = plan
                .assignments
                .iter()
                .filter(|a| a.task_kind == kind && a.cohort.requested_model == model)
                .map(|a| a.input_id.as_str())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            let id = |index: usize, workflow: Workflow| {
                plan.assignments
                    .iter()
                    .find(|a| {
                        a.input_id == inputs[index]
                            && a.cohort.requested_model == model
                            && a.cohort.workflow == workflow
                    })
                    .unwrap()
                    .id
                    .clone()
            };
            for (existing_first, qualitygate_first) in [(0, 2), (1, 3)] {
                plan.run_order.extend([
                    id(existing_first, Workflow::ExistingTools),
                    id(qualitygate_first, Workflow::Qualitygate),
                    id(qualitygate_first, Workflow::ExistingTools),
                    id(existing_first, Workflow::Qualitygate),
                ]);
            }
        }
    }
    assert_eq!(plan.assignments.len(), 32);
    let sealed = seal(plan, 1).unwrap();
    assert_eq!(sealed.run_order.len(), 32);
}
