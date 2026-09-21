mod common;
use common::*;
use qualitygate::snapshot::{self, CaptureOptions, Selection};
use std::time::{Duration, Instant};

#[test]
fn legacy_large_blobs_are_acquired_explicitly_without_hiding_policy_or_source_changes() {
    let directory = fixture();
    let root = directory.path();
    report(&cli(root, &["init", "--format", "json"]), 0);
    std::fs::write(root.join("legacy.bin"), vec![0; 5 * 1024 * 1024]).unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "legacy baseline"]);
    std::fs::write(root.join("README.md"), "documentation change\n").unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "README only"]);
    for selection in [
        vec!["--worktree"],
        vec!["--staged"],
        vec!["--diff", "HEAD~1..HEAD"],
    ] {
        let mut args = vec!["check", "--profile", "full", "--format", "json"];
        args.extend(selection);
        let incomplete = report(&cli(root, &args), 2);
        assert!(
            incomplete["gate"]["blockers"]
                .to_string()
                .contains("--snapshot-max-file-mib 5")
        );
        args.extend(["--snapshot-max-file-mib", "5"]);
        let passed = report(&cli(root, &args), 0);
        assert_eq!(passed["gate"]["complete"], true);
        assert_eq!(passed["scope"], "repository");
        assert!(
            !passed["snapshot"]["content_digest"]
                .as_str()
                .unwrap()
                .is_empty()
        );
    }
    let insufficient = report(
        &cli(
            root,
            &[
                "check",
                "--path",
                "README.md",
                "--snapshot-max-mib",
                "1024",
                "--format",
                "json",
            ],
        ),
        2,
    );
    assert!(
        insufficient["gate"]["blockers"]
            .to_string()
            .contains("legacy.bin")
    );
    let total = report(
        &cli(
            root,
            &[
                "check",
                "--snapshot-max-file-mib",
                "5",
                "--snapshot-max-mib",
                "4",
                "--format",
                "json",
            ],
        ),
        2,
    );
    assert!(
        total["gate"]["blockers"]
            .to_string()
            .contains("Snapshot exceeds")
    );
    // A larger acquisition budget is not permission to weaken the selected policy.
    std::fs::write(
        root.join("qualitygate.yaml"),
        "schema_version: 1\nrules:\n  line-ending: {enabled: false}\n",
    )
    .unwrap();
    let changed = report(
        &cli(
            root,
            &[
                "check",
                "--policy-ref",
                "HEAD",
                "--snapshot-max-file-mib",
                "5",
                "--format",
                "json",
            ],
        ),
        2,
    );
    assert_eq!(
        changed["policy"]["changes"],
        serde_json::json!(["qualitygate.yaml"])
    );
    git(root, &["restore", "qualitygate.yaml"]);
    std::fs::write(root.join("bad.txt"), "bad\r\n").unwrap();
    let failed = report(
        &cli(
            root,
            &["check", "--snapshot-max-file-mib", "5", "--format", "json"],
        ),
        1,
    );
    assert_eq!(failed["gate"]["complete"], true);
    let argv = failed["checks"][0]["diagnostics"][0]["recheck"]["argv"]
        .as_array()
        .unwrap();
    let position = argv
        .iter()
        .position(|value| value == "--snapshot-max-file-mib")
        .unwrap();
    assert_eq!(argv[position + 1], "5");
}

#[test]
fn raised_file_budget_preserves_full_snapshot_bytes_and_digest_under_path_filtering() {
    let directory = fixture();
    let root = directory.path();
    let bytes = vec![b'x'; qualitygate::domain::snapshot_budget::MAX_FILE_BYTES];
    std::fs::write(root.join("large.bin"), &bytes).unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "maximum size blob"]);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let options = CaptureOptions {
        max_file_bytes: bytes.len(),
        path_filter: Some("hello.txt".into()),
        ..CaptureOptions::default()
    };
    let captured = runtime
        .block_on(snapshot::capture_with_options(
            root,
            &Selection::Staged,
            &options,
        ))
        .unwrap();
    assert_eq!(captured.files["large.bin"].bytes, bytes);
    assert!(!captured.includes("large.bin"));
    assert_eq!(
        captured.identity.content_digest,
        snapshot::content_digest(&captured.files)
    );
    std::fs::write(root.join("large.bin"), vec![b'x'; bytes.len() + 1]).unwrap();
    let error = runtime
        .block_on(snapshot::capture_with_options(
            root,
            &Selection::Worktree {
                base: "HEAD".into(),
            },
            &options,
        ))
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("supported single-file maximum is 8 MiB")
    );
}

#[test]
fn eighteen_thousand_files_with_small_diff_support_all_selectors_and_bounded_parallelism() {
    let directory = fixture();
    let root = directory.path();
    std::fs::write(
        root.join("qualitygate.yaml"),
        "schema_version: 1\nrules:\n  line-ending: {enabled: true}\n",
    )
    .unwrap();
    std::fs::create_dir(root.join("bulk")).unwrap();
    // Unique blobs prevent object-cache/deduplication from hiding acquisition cost.
    for index in 0..18_000 {
        let text = format!("{index:05} {}\n", "x".repeat(2040));
        std::fs::write(root.join(format!("bulk/file-{index:05}.txt")), text).unwrap();
    }
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "large baseline"]);
    std::fs::write(root.join("changed.txt"), "small\r\n").unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "small delta"]);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let diff = runtime
        .block_on(snapshot::run_git(
            root,
            &["diff", "HEAD~1..HEAD", "--"],
            None,
        ))
        .unwrap();
    assert!(diff.len() < 10 * 1024);
    let mut expected = None;
    for jobs in [1, 4, 4, 1] {
        let start = Instant::now();
        let captured = runtime
            .block_on(snapshot::capture_with_options(
                root,
                &Selection::Diff {
                    base: "HEAD~1".into(),
                    head: "HEAD".into(),
                },
                &CaptureOptions {
                    jobs,
                    ..CaptureOptions::default()
                },
            ))
            .unwrap();
        let elapsed = start.elapsed();
        eprintln!("issue2 diff: 18003 files, 35+ MiB, jobs={jobs}, elapsed={elapsed:?}");
        assert!(
            elapsed < Duration::from_secs(60),
            "acquisition exceeded large-repository regression budget: {elapsed:?}"
        );
        assert_eq!(captured.files.len(), 18_003);
        assert!(
            captured
                .files
                .values()
                .map(|file| file.bytes.len())
                .sum::<usize>()
                > 32 * 1024 * 1024
        );
        assert_eq!(captured.changes.len(), 1);
        assert_eq!(captured.changes["changed.txt"].added_lines, [1].into());
        let evidence = (
            captured.identity.content_digest,
            serde_json::to_value(captured.changes).unwrap(),
            captured.commits,
        );
        if let Some(expected) = &expected {
            assert_eq!(&evidence, expected);
        } else {
            expected = Some(evidence);
        }
    }
    // The committed delta is small while every mode must retain the full context.
    std::fs::write(root.join("changed.txt"), "staged\n").unwrap();
    git(root, &["add", "."]);
    std::fs::write(root.join("changed.txt"), "worktree\n").unwrap();
    for selection in [
        vec!["--staged"],
        vec!["--worktree"],
        vec!["--diff", "HEAD~1..HEAD"],
        vec!["--path", "changed.txt"],
    ] {
        let start = Instant::now();
        let mut args = vec!["check", "--profile", "quick", "--format", "json"];
        args.extend(selection);
        let result = report(&cli(root, &args), 1);
        assert_eq!(result["gate"]["complete"], true);
        assert_eq!(result["checks"][0]["diagnostics"][0]["file"], "changed.txt");
        assert!(start.elapsed() < Duration::from_secs(120));
    }
    let incomplete = report(
        &cli(
            root,
            &[
                "check",
                "--staged",
                "--snapshot-max-mib",
                "16",
                "--format",
                "json",
            ],
        ),
        2,
    );
    assert_eq!(incomplete["gate"]["complete"], false);
    assert!(
        incomplete["gate"]["blockers"]
            .to_string()
            .contains("Snapshot exceeds 16777216 bytes")
    );
}

#[test]
fn path_combines_with_selectors_preserves_bytes_and_recheck_options() {
    let root = fixture();
    let root = root.path();
    std::fs::write(
        root.join("qualitygate.yaml"),
        "schema_version: 1\nrules:\n  line-ending: {enabled: true}\n",
    )
    .unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "policy"]);
    std::fs::write(root.join("hello.txt"), "committed\r\n").unwrap();
    git(root, &["commit", "-qam", "delta"]);
    std::fs::write(root.join("hello.txt"), "staged\n").unwrap();
    git(root, &["add", "."]);
    std::fs::write(root.join("hello.txt"), "working\r\n").unwrap();
    std::fs::write(root.join("outside.txt"), "outside\r\n").unwrap();
    for (selection, code, mode) in [
        (vec!["--staged"], 1, "staged"),
        (vec!["--worktree"], 0, "worktree"),
        (vec!["--diff", "HEAD~1..HEAD"], 1, "diff"),
    ] {
        let mut args = vec![
            "check",
            "--path",
            "hello.txt",
            "--snapshot-jobs",
            "2",
            "--snapshot-max-mib",
            "32",
            "--snapshot-max-file-mib",
            "8",
            "--snapshot-timeout-secs",
            "60",
            "--format",
            "json",
        ];
        args.extend(selection);
        let result = report(&cli(root, &args), code);
        assert_eq!(result["snapshot"]["mode"], mode);
        assert_eq!(result["scope"], "path");
        if code == 1 {
            let argv = result["checks"][0]["diagnostics"][0]["recheck"]["argv"]
                .as_array()
                .unwrap();
            for (flag, value) in [
                ("--path", "hello.txt"),
                ("--snapshot-jobs", "2"),
                ("--snapshot-max-mib", "32"),
                ("--snapshot-max-file-mib", "8"),
                ("--snapshot-timeout-secs", "60"),
            ] {
                assert_eq!(argv.iter().filter(|arg| *arg == flag).count(), 1);
                let index = argv.iter().position(|arg| arg == flag).unwrap();
                assert_eq!(argv[index + 1], value);
            }
        }
    }
    for args in [
        vec!["--snapshot-jobs", "0"],
        vec!["--snapshot-jobs", "17"],
        vec!["--snapshot-max-mib", "1025"],
        vec!["--snapshot-max-file-mib", "0"],
        vec!["--snapshot-max-file-mib", "9"],
        vec!["--snapshot-timeout-secs", "0"],
        vec!["--staged", "--worktree"],
    ] {
        let mut command = vec!["check"];
        command.extend(args);
        assert_eq!(cli(root, &command).status.code(), Some(2));
    }
    std::fs::write(
        root.join("qualitygate.yaml"),
        "schema_version: 1\nrules:\n  line-ending: {enabled: false}\n",
    )
    .unwrap();
    let changed_policy = report(
        &cli(
            root,
            &[
                "check",
                "--worktree",
                "--path",
                "hello.txt",
                "--policy-ref",
                "HEAD",
                "--format",
                "json",
            ],
        ),
        2,
    );
    assert_eq!(
        changed_policy["policy"]["changes"],
        serde_json::json!(["qualitygate.yaml"])
    );
    assert_eq!(changed_policy["gate"]["complete"], false);
}
