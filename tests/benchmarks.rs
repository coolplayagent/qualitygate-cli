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

#[test]
fn duplicate_content_rename_storm_consumes_each_old_path_once() {
    use qualitygate::snapshot::{File, compare_files};
    use std::collections::BTreeMap;
    let file = File {
        bytes: b"identical content\n".to_vec(),
        executable: false,
    };
    let base: BTreeMap<_, _> = (0..18_000)
        .map(|index| (format!("old/{index:05}"), file.clone()))
        .collect();
    let mut head: BTreeMap<_, _> = (0..18_000)
        .map(|index| (format!("new/{index:05}"), file.clone()))
        .collect();
    head.insert("surplus".into(), file);
    let started = std::time::Instant::now();
    let changes = compare_files(&base, &head);
    assert!(started.elapsed() < std::time::Duration::from_secs(10));
    assert_eq!(changes.len(), 18_001);
    for index in 0..18_000 {
        let change = &changes[&format!("new/{index:05}")];
        assert_eq!(change.kind, "renamed");
        assert_eq!(change.old_path, Some(format!("old/{index:05}")));
        assert!(change.added_lines.is_empty());
    }
    assert_eq!(changes["surplus"].kind, "added");
    assert_eq!(changes["surplus"].old_path, None);
    assert_eq!(changes["surplus"].added_lines, [1].into());
}
