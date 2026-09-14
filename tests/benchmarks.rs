use qualitygate::domain::{CheckResult, Decision, Severity, evaluate};

#[test]
fn gate_handles_ten_thousand_independent_results_deterministically() {
    let mut results = Vec::new();
    for index in 0..10_000 {
        let mut result = CheckResult::pending(format!("check-{index}"), true, Severity::Error);
        result.complete();
        results.push(result);
    }
    let started = std::time::Instant::now();
    let gate = evaluate(&results, &[], &[]);
    assert_eq!(gate.decision, Decision::Pass);
    assert!(started.elapsed() < std::time::Duration::from_secs(5));
    assert!(gate.blockers.is_empty());
}

#[test]
fn rename_storm_preserves_lines_and_deterministic_old_paths() {
    use qualitygate::snapshot::{File, compare_files};
    use std::collections::BTreeMap;
    let mut base = BTreeMap::new();
    let mut head = BTreeMap::new();
    for index in 0..18_000 {
        let file = File {
            bytes: format!("unique content {index}\n").into_bytes(),
            executable: false,
        };
        base.insert(format!("old/{index:05}"), file.clone());
        head.insert(format!("new/{index:05}"), file);
    }
    let started = std::time::Instant::now();
    let changes = compare_files(&base, &head);
    assert!(started.elapsed() < std::time::Duration::from_secs(10));
    assert_eq!(changes.len(), 18_000);
    for (name, change) in changes {
        assert_eq!(change.kind, "renamed");
        assert!(change.added_lines.is_empty());
        assert_eq!(change.old_path, Some(name.replace("new/", "old/")));
    }
}
