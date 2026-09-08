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
