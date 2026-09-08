use super::*;
use crate::{
    adapters::reports::{CoverageLine, Tests},
    domain::Severity,
    snapshot::{Change, File, Identity},
};

fn snapshot() -> Snapshot {
    let file = || File {
        bytes: b"old\nchanged\nstable\n".to_vec(),
        executable: false,
    };
    Snapshot {
        root: "/repo".into(),
        identity: Identity {
            merge_request: None,
            mode: "worktree".into(),
            base: "base".into(),
            head: "head".into(),
            content_digest: "digest".into(),
        },
        files: [("src/a.rs".into(), file()), ("src/b.rs".into(), file())].into(),
        base_files: [("src/a.rs".into(), file()), ("src/b.rs".into(), file())].into(),
        changes: [(
            "src/a.rs".into(),
            Change {
                kind: "modified".into(),
                old_path: Some("src/a.rs".into()),
                added_lines: [2].into(),
            },
        )]
        .into(),
        path_filter: None,
        commits: Vec::new(),
    }
}

fn spec(mode: &str) -> ReportSpec {
    serde_norway::from_str(&format!(
        "path: report\nformat: diagnostics\nmode: {mode}\n"
    ))
    .unwrap()
}
fn pending() -> CheckResult {
    CheckResult::pending("analyzer", true, Severity::Error)
}
fn issue(file: &str, line: usize) -> Issue {
    Issue {
        rule: "rule".into(),
        file: Some(file.into()),
        line: Some(line),
        symbol: Some("symbol".into()),
        message: "real finding".into(),
    }
}
fn data(issues: Vec<Issue>) -> Data {
    Data {
        issues,
        ..Data::default()
    }
}
fn coverage(records: &[(&str, usize, u64, usize, usize)]) -> Data {
    Data {
        coverage: records
            .iter()
            .map(|(file, line, hits, found, hit)| CoverageLine {
                file: (*file).into(),
                line: *line,
                hits: *hits,
                branches_found: *found,
                branches_hit: *hit,
            })
            .collect(),
        branch_coverage: true,
        ..Data::default()
    }
}
fn coverage_spec(mode: &str) -> ReportSpec {
    let mut spec = spec(mode);
    spec.coverage_paths = vec!["src/*.rs".into()];
    spec.minimum_coverage = Some(90.0);
    spec
}
fn apply_data(
    result: &mut CheckResult,
    spec: &ReportSpec,
    data: Data,
    baseline: Option<Data>,
) -> Result<()> {
    apply(
        result,
        spec,
        data,
        baseline,
        &snapshot(),
        Path::new("/checked"),
    )
}

#[test]
fn changed_lines_filters_old_issues_and_rejects_unlocated_diagnostics() {
    let mut result = pending();
    apply_data(
        &mut result,
        &spec("changed_lines"),
        data(vec![issue("src/a.rs", 1), issue("src/a.rs", 2)]),
        None,
    )
    .unwrap();
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].range.as_ref().unwrap().start_line, 2);
    assert_eq!(result.metadata["report:filtered"], 1);
    let mut unknown = issue("src/a.rs", 1);
    unknown.line = None;
    assert!(
        apply_data(
            &mut pending(),
            &spec("changed_lines"),
            data(vec![unknown.clone()]),
            None
        )
        .is_err()
    );
    unknown.file = None;
    assert!(
        apply_data(
            &mut pending(),
            &spec("changed_lines"),
            data(vec![unknown]),
            None
        )
        .is_err()
    );
}

#[test]
fn new_diagnostics_match_multiplicity_and_survive_line_and_file_moves() {
    let mut result = pending();
    apply_data(
        &mut result,
        &spec("new_diagnostics"),
        data(vec![issue("src/a.rs", 2), issue("src/a.rs", 3)]),
        Some(data(vec![issue("src/a.rs", 1)])),
    )
    .unwrap();
    assert_eq!(result.diagnostics.len(), 1);
    assert!(
        apply_data(
            &mut pending(),
            &spec("new_diagnostics"),
            Data::default(),
            None
        )
        .is_err()
    );
    let mut snapshot = snapshot();
    snapshot.files.remove("src/b.rs");
    snapshot.base_files.remove("src/a.rs");
    let change = snapshot.changes.get_mut("src/a.rs").unwrap();
    change.kind = "renamed".into();
    change.old_path = Some("src/b.rs".into());
    let mut result = pending();
    apply(
        &mut result,
        &spec("new_diagnostics"),
        data(vec![issue("src/a.rs", 2)]),
        Some(data(vec![issue("src/b.rs", 1)])),
        &snapshot,
        Path::new("/checked"),
    )
    .unwrap();
    assert!(result.diagnostics.is_empty());
}

#[test]
fn affected_scope_includes_unchanged_files_and_requires_explicit_impact_evidence() {
    let mut findings = data(vec![issue("src/a.rs", 2), issue("src/b.rs", 3)]);
    assert!(
        apply_data(
            &mut pending(),
            &spec("affected_scope"),
            findings.clone(),
            None
        )
        .is_err()
    );
    findings.affected_files = Some(vec!["src/b.rs".into()]);
    let mut result = pending();
    apply_data(&mut result, &spec("affected_scope"), findings, None).unwrap();
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].file.as_deref(), Some("src/b.rs"));
}

#[test]
fn report_locations_must_map_uniquely_to_checked_sources() {
    let snapshot = snapshot();
    for value in [
        "a.rs",
        "src/a.rs",
        "/checked/src/a.rs",
        "file:///checked/src/a.rs",
        "/repo/src/a.rs",
    ] {
        assert_eq!(
            map_file(value, &snapshot, Path::new("/checked"), false).unwrap(),
            "src/a.rs"
        );
    }
    assert!(map_file("../a.rs", &snapshot, Path::new("/checked"), false).is_err());
    let mut ambiguous = snapshot.clone();
    ambiguous
        .files
        .insert("other/a.rs".into(), ambiguous.files["src/a.rs"].clone());
    assert!(map_file("a.rs", &ambiguous, Path::new("/checked"), false).is_err());
    assert!(map_file("missing.rs", &snapshot, Path::new("/checked"), false).is_err());
}

#[test]
fn test_statistics_cannot_claim_success_with_failures_or_missing_counts() {
    let mut spec = spec("full");
    spec.minimum_tests = Some(1);
    assert!(apply_data(&mut pending(), &spec, Data::default(), None).is_err());
    for (executed, failures) in [(0, 0), (2, 1)] {
        let data = Data {
            tests: Some(Tests {
                executed,
                failures,
                skipped: 0,
            }),
            ..Data::default()
        };
        let mut result = pending();
        apply_data(&mut result, &spec, data, None).unwrap();
        assert_eq!(result.diagnostics.len(), 1);
    }
}

#[test]
fn coverage_rejects_omitted_sources_duplicate_aliases_invalid_lines_and_missing_branches() {
    let spec = coverage_spec("full");
    let incomplete = coverage(&[("src/a.rs", 2, 1, 0, 0)]);
    assert!(
        apply_data(&mut pending(), &spec, incomplete.clone(), None)
            .unwrap_err()
            .to_string()
            .contains("omitted")
    );
    let mut duplicate = incomplete.clone();
    duplicate.coverage.push(CoverageLine {
        file: "a.rs".into(),
        ..duplicate.coverage[0].clone()
    });
    assert!(
        apply_data(&mut pending(), &spec, duplicate, None)
            .unwrap_err()
            .to_string()
            .contains("Duplicate")
    );
    let mut invalid = incomplete.clone();
    invalid.coverage[0].line = 200;
    assert!(apply_data(&mut pending(), &spec, invalid, None).is_err());
    let mut no_branch = incomplete;
    no_branch.branch_coverage = false;
    assert!(
        apply_data(&mut pending(), &spec, no_branch, None)
            .unwrap_err()
            .to_string()
            .contains("branch coverage")
    );
    assert!(apply_data(&mut pending(), &spec, Data::default(), None).is_err());
}

#[test]
fn changed_coverage_uses_both_line_and_branch_thresholds_and_records_scope() {
    let spec = coverage_spec("changed_lines");
    let records = coverage(&[
        ("src/a.rs", 1, 0, 0, 0),
        ("src/a.rs", 2, 1, 2, 1),
        ("src/b.rs", 1, 0, 0, 0),
    ]);
    let mut result = pending();
    apply_data(&mut result, &spec, records.clone(), None).unwrap();
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.metadata["report:coverage"]["line_percent"], 100.0);
    assert_eq!(result.metadata["report:coverage"]["branch_percent"], 50.0);
    assert_eq!(
        result.metadata["report:coverage"]["source_files"],
        serde_json::json!(["src/a.rs"])
    );
    let mut lines_only = spec;
    lines_only.require_branch_coverage = false;
    let mut result = pending();
    apply_data(&mut result, &lines_only, records, None).unwrap();
    assert!(result.diagnostics.is_empty());
    let mut result = pending();
    apply_data(
        &mut result,
        &coverage_spec("full"),
        coverage(&[("src/a.rs", 1, 0, 0, 0), ("src/b.rs", 1, 1, 0, 0)]),
        None,
    )
    .unwrap();
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.metadata["report:coverage"]["line_percent"], 50.0);
}

#[test]
fn zero_selected_executable_lines_are_reported_as_null_rates_not_hundred_percent() {
    let mut result = pending();
    let data = Data {
        coverage_files: vec!["src/a.rs".into(), "src/b.rs".into()],
        branch_coverage: true,
        ..Data::default()
    };
    apply_data(&mut result, &coverage_spec("changed_lines"), data, None).unwrap();
    assert_eq!(
        result.metadata["report:coverage"]["line_percent"],
        serde_json::Value::Null
    );
    assert_eq!(
        result.metadata["report:coverage"]["branch_percent"],
        serde_json::Value::Null
    );
    assert_eq!(
        result.metadata["report:coverage"]["no_executable_lines_selected"],
        true
    );
}
