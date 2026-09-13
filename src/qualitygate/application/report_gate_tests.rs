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
        tool: None,
        locations: vec![],
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
                excluded: false,
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

#[test]
fn coverage_source_roots_resolve_exactly_and_exclusions_remain_snapshot_bound() {
    let mut data = coverage(&[("a.rs", 2, 0, 0, 0), ("b.rs", 1, 1, 0, 0)]);
    data.coverage_files = vec!["a.rs".into(), "b.rs".into()];
    data.coverage[0].excluded = true;
    for root in ["src", "/checked/src", "/repo/src"] {
        data.coverage_roots = vec![root.into()];
        let mut result = pending();
        apply_data(
            &mut result,
            &coverage_spec("changed_lines"),
            data.clone(),
            None,
        )
        .unwrap();
        assert!(result.diagnostics.is_empty());
        assert_eq!(result.metadata["report:coverage"]["excluded_lines"], 1);
        assert_eq!(result.metadata["report:coverage"]["lines"], 0);
    }
    for root in ["", "missing", "/foreign/src", "../src"] {
        data.coverage_roots = vec![root.into()];
        assert!(
            apply_data(&mut pending(), &coverage_spec("full"), data.clone(), None).is_err(),
            "{root}"
        );
    }
    data.coverage_roots = vec!["src".into()];
    data.coverage[0].line = 9;
    assert!(apply_data(&mut pending(), &coverage_spec("full"), data.clone(), None).is_err());
    data.coverage[0].line = 2;
    data.coverage[0].hits = 1;
    assert!(apply_data(&mut pending(), &coverage_spec("full"), data.clone(), None).is_err());
    data.coverage[0].hits = 0;
    let mut snapshot = snapshot();
    snapshot
        .files
        .insert("tests/a.rs".into(), snapshot.files["src/a.rs"].clone());
    data.coverage_roots.push("tests".into());
    assert!(
        apply(
            &mut pending(),
            &coverage_spec("full"),
            data,
            None,
            &snapshot,
            Path::new("/checked")
        )
        .is_err()
    );
}

#[test]
fn short_report_names_cannot_expand_unboundedly_when_mapped_to_long_source_paths() {
    let mut snapshot = snapshot();
    snapshot.files.clear();
    snapshot.files.insert(
        format!("src/{}/a.rs", "a".repeat(4000)),
        File {
            bytes: b"statement\n".repeat(1000),
            executable: false,
        },
    );
    let records = (1..=1000)
        .map(|line| ("a.rs", line, 1, 0, 0))
        .collect::<Vec<_>>();
    let error = apply(
        &mut pending(),
        &coverage_spec("full"),
        coverage(&records),
        None,
        &snapshot,
        Path::new("/checked"),
    )
    .unwrap_err();
    assert!(error.to_string().contains("16 MiB"));
}

#[test]
fn multiple_locations_use_any_matching_range_and_keep_a_stable_identity() {
    let mut finding = issue("src/b.rs", 1);
    finding.tool = Some("scan".into());
    finding.locations = vec![
        IssueLocation {
            file: finding.file.clone(),
            line: finding.line,
            end_line: None,
            symbol: finding.symbol.clone(),
        },
        IssueLocation {
            file: Some("src/a.rs".into()),
            line: Some(1),
            end_line: Some(3),
            symbol: Some("other".into()),
        },
    ];
    let mut full = pending();
    apply_data(&mut full, &spec("full"), data(vec![finding.clone()]), None).unwrap();
    let mut changed = pending();
    apply_data(
        &mut changed,
        &spec("changed_lines"),
        data(vec![finding.clone()]),
        None,
    )
    .unwrap();
    assert_eq!(changed.diagnostics[0].file.as_deref(), Some("src/a.rs"));
    assert_eq!(changed.diagnostics[0].range.as_ref().unwrap().end_line, 3);
    assert_eq!(
        full.diagnostics[0].fingerprint,
        changed.diagnostics[0].fingerprint
    );
    let mut affected = data(vec![finding.clone()]);
    affected.affected_files = Some(vec!["src/a.rs".into()]);
    let mut result = pending();
    apply_data(&mut result, &spec("affected_scope"), affected, None).unwrap();
    assert_eq!(result.diagnostics[0].file.as_deref(), Some("src/a.rs"));
    finding.locations.reverse();
    finding.file = finding.locations[0].file.clone();
    finding.line = finding.locations[0].line;
    finding.symbol = finding.locations[0].symbol.clone();
    assert_eq!(
        fingerprint(&finding),
        fingerprint(&data_from_evidence(&full))
    );
}

fn data_from_evidence(result: &CheckResult) -> Issue {
    serde_json::from_value(result.diagnostics[0].evidence.clone()).unwrap()
}

#[test]
fn unresolved_locations_and_impossible_source_ranges_cannot_disappear() {
    let mut finding = issue("src/b.rs", 1);
    finding.locations = vec![
        IssueLocation {
            file: finding.file.clone(),
            line: finding.line,
            end_line: None,
            symbol: finding.symbol.clone(),
        },
        IssueLocation {
            file: None,
            line: None,
            end_line: None,
            symbol: Some("unmapped".into()),
        },
    ];
    assert!(
        apply_data(
            &mut pending(),
            &spec("changed_lines"),
            data(vec![finding.clone()]),
            None
        )
        .is_err()
    );
    let mut affected = data(vec![finding.clone()]);
    affected.affected_files = Some(vec!["src/a.rs".into()]);
    assert!(apply_data(&mut pending(), &spec("affected_scope"), affected, None).is_err());
    // A proven matching location suffices even when a different one is unlocated.
    finding.file = Some("src/a.rs".into());
    finding.locations[0].file = Some("src/a.rs".into());
    finding.locations[0].end_line = Some(2);
    apply_data(
        &mut pending(),
        &spec("changed_lines"),
        data(vec![finding.clone()]),
        None,
    )
    .unwrap();
    for (line, end) in [(0, None), (4, None), (2, Some(1)), (1, Some(4))] {
        finding.locations[0].line = Some(line);
        finding.locations[0].end_line = end;
        assert!(
            apply_data(
                &mut pending(),
                &spec("full"),
                data(vec![finding.clone()]),
                None
            )
            .is_err()
        );
    }
}

#[test]
fn producer_identity_without_locations_still_includes_primary_path_and_symbol() {
    let mut finding = issue("src/a.rs", 1);
    finding.tool = Some("scan".into());
    let identity = fingerprint(&finding);
    finding.line = Some(2);
    assert_eq!(identity, fingerprint(&finding));
    finding.file = Some("src/b.rs".into());
    assert_ne!(identity, fingerprint(&finding));
    finding.file = Some("src/a.rs".into());
    finding.tool = Some("different scan".into());
    assert_ne!(identity, fingerprint(&finding));
}
