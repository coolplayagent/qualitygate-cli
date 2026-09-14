mod common;
#[path = "common/evolution.rs"]
mod evolution;

use evolution::*;
use std::time::{Duration, Instant};

#[test]
fn paired_large_repository_shares_snapshots_and_preserves_serial_parallel_results() {
    let mut fixture = Fixture::new();
    let bulk = fixture.root.path().join("bulk");
    std::fs::create_dir(&bulk).unwrap();
    for index in 0..18_000 {
        std::fs::write(
            bulk.join(format!("input-{index:05}.txt")),
            format!("{index}\n{}\n", "x".repeat(2048)),
        )
        .unwrap();
    }
    common::git(fixture.root.path(), &["add", "."]);
    common::git(
        fixture.root.path(),
        &["commit", "-qm", "large immutable shared input"],
    );
    for (index, case) in fixture.suite.cases.iter_mut().enumerate() {
        case.base = head(fixture.root.path());
        std::fs::write(
            fixture.root.path().join(format!("paired-{index}.txt")),
            if index == 0 { "bad\r\n" } else { "good\n" },
        )
        .unwrap();
        common::git(fixture.root.path(), &["add", "."]);
        common::git(
            fixture.root.path(),
            &["commit", "-qm", "independent large input case"],
        );
        case.head = head(fixture.root.path());
    }
    let budget = &mut fixture.suite.budget;
    budget.snapshot_max_mib = 128;
    budget.snapshot_jobs = 4;
    budget.snapshot_timeout_seconds = 90;
    budget.max_live_snapshot_mib = 512;
    budget.case_timeout_seconds = 120;
    budget.total_timeout_seconds = 360;
    fixture.write_inputs();
    fixture.enable();
    let start = Instant::now();
    let serial = fixture.validate("1", 0);
    let serial_time = start.elapsed();
    let start = Instant::now();
    let parallel = fixture.validate("4", 0);
    let parallel_time = start.elapsed();
    assert!(serial_time < Duration::from_secs(360));
    assert!(parallel_time < Duration::from_secs(360));
    assert_eq!(
        serial["evaluation"]["budget"],
        parallel["evaluation"]["budget"]
    );
    for (left, right) in serial["evaluation"]["cases"]
        .as_array()
        .unwrap()
        .iter()
        .zip(parallel["evaluation"]["cases"].as_array().unwrap())
    {
        assert_eq!(left["snapshot_digest"], right["snapshot_digest"]);
        assert_eq!(left["task_digest"], right["task_digest"]);
        for side in ["baseline", "candidate"] {
            for field in [
                "complete",
                "mismatches",
                "selected_rules",
                "activated_rules",
                "completed_checks",
                "findings",
            ] {
                assert_eq!(left[side][field], right[side][field]);
            }
        }
    }
    println!(
        "Paired 18,000-file input (>35 MiB), three cases: serial={:.3}s parallel={:.3}s; global check jobs=1/4; snapshot readers=4/case; reserved live snapshot bytes<=512 MiB. Shared-host timings, no universal speedup claim.",
        serial_time.as_secs_f64(),
        parallel_time.as_secs_f64()
    );
}
