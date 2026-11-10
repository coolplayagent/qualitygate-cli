use super::*;
#[tokio::test]
async fn expired_deadline_is_typed_and_prevents_process_launch() {
    let deadline = Instant::now() - Duration::from_millis(1);
    let error = command(
        &["missing-agent-binary".into()],
        Path::new("."),
        None,
        deadline,
    )
    .await
    .unwrap_err();
    assert!(error.is::<TimeBudgetExpired>());
}
#[test]
fn budget_errors_keep_pre_agent_attempts_distinct_from_other_execution_errors() {
    let mut ledger = json!({"attempts":[{"status":"running"}]});
    record_execution_error(&mut ledger, &TimeBudgetExpired.into());
    assert_eq!(ledger["termination"], "time_budget");
    assert_eq!(
        ledger["attempts"][0]["status"],
        "budget_exhausted_before_agent"
    );
    let mut initial = json!({"attempts":[]});
    record_execution_error(&mut initial, &TimeBudgetExpired.into());
    assert_eq!(initial["termination"], "time_budget");
    assert_eq!(initial["attempts"].as_array().unwrap().len(), 0);
    record_execution_error(&mut initial, &anyhow::anyhow!("invalid report"));
    assert_eq!(initial["termination"], "execution_incomplete");
}
#[test]
fn changed_or_repeated_diagnostics_do_not_reset_the_no_progress_budget() {
    let before = BTreeSet::from(["a".into(), "b".into()]);
    assert!(progress(&before, &BTreeSet::from(["a".into()])));
    assert!(!progress(&before, &before));
    assert!(!progress(&before, &BTreeSet::from(["new".into()])));
    assert!(!progress(&BTreeSet::new(), &BTreeSet::new()));
}
#[test]
fn files_are_bounded() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("input");
    assert!(read(&path, 10).is_err());
    std::fs::write(&path, b"12345678901").unwrap();
    assert!(read(&path, 10).is_err());
    assert_eq!(read(&path, 11).unwrap().len(), 11);
}

#[test]
fn read_only_proposals_cannot_redirect_or_overwrite_changed_inputs() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("src")).unwrap();
    let path = Path::new("src/lib.rs");
    std::fs::write(root.path().join(path), "old").unwrap();
    for name in [
        "../outside",
        "src/../task.yaml",
        ".git/config",
        "src/missing.rs",
    ] {
        assert!(replacement_target(root.path(), Path::new(name)).is_err());
    }
    for proposal in [
        json!({"path":"src/other.rs","content":"new"}),
        json!({"path":"src/lib.rs","content":"new","unrequested":true}),
        json!({"path":"src/lib.rs","content":"x".repeat(1024 * 1024 + 1)}),
    ] {
        assert!(apply_replacement(root.path(), path, b"old", &proposal.to_string()).is_err());
    }
    let proposal = json!({"path":"src/lib.rs","content":"new"}).to_string();
    assert!(apply_replacement(root.path(), path, b"stale", &proposal).is_err());
    let applied = apply_replacement(root.path(), path, b"old", &proposal).unwrap();
    assert_ne!(applied["before"], applied["after"]);
    assert_eq!(std::fs::read(root.path().join(path)).unwrap(), b"new");
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("lib.rs", root.path().join("src/link.rs")).unwrap();
        assert!(replacement_target(root.path(), Path::new("src/link.rs")).is_err());
    }
}

pub(super) fn options(root: &Path, external: &Path) -> Options {
    std::fs::write(external.join("agent.json"), b"[\"git\",\"--version\"]").unwrap();
    std::fs::write(external.join("prompt.txt"), b"Repair the declared task").unwrap();
    Options {
        root: root.into(),
        config: "qualitygate.yaml".into(),
        qualitygate: std::env::current_exe().unwrap(),
        base: "HEAD".into(),
        policy_ref: "HEAD".into(),
        task: "task.yaml".into(),
        prompt_file: external.join("prompt.txt"),
        agent_command: external.join("agent.json"),
        replacement_file: None,
        output_dir: external.into(),
        feedback_mode: FeedbackMode::Full,
        max_attempts: 2,
        timeout_seconds: 10,
        role: "implementer".into(),
        resume: None,
        trust_store: None,
        evidence_dir: None,
    }
}

#[test]
fn resume_is_context_only_and_cannot_reuse_authority_or_budget() {
    let root = tempfile::tempdir().unwrap();
    let external = tempfile::tempdir().unwrap();
    let mut options = options(root.path(), external.path());
    assert!(
        contract::resume(&options, external.path())
            .unwrap()
            .is_none()
    );
    let previous = json!({"kind":"external_agent_loop","root":root.path(),
        "attempts":[{},{}],"agent_argv":["old-agent"],"termination":"界".repeat(300),
        "contract_digest":"a".repeat(200),"complete":true,"budget":{"attempts":99}});
    let path = external.path().join("previous.json");
    std::fs::write(&path, serde_json::to_vec(&previous).unwrap()).unwrap();
    options.resume = Some(path.clone());
    let resumed = contract::resume(&options, external.path())
        .unwrap()
        .unwrap();
    assert_eq!(resumed["attempts"], 2);
    assert_eq!(resumed["agent_changed"], true);
    assert_eq!(resumed["evidence_reused"], false);
    assert_eq!(resumed["budget"], "fresh explicit invocation");
    assert_eq!(
        resumed["termination"].as_str().unwrap().chars().count(),
        256
    );
    assert_eq!(resumed["previous_contract"].as_str().unwrap().len(), 128);
    let retained: Value = serde_json::from_slice(
        &std::fs::read(external.path().join("resume-context.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(retained, previous);
    for invalid in [
        json!({"kind":"other","root":root.path()}),
        json!({"kind":"external_agent_loop","root":"/other"}),
    ] {
        std::fs::write(&path, invalid.to_string()).unwrap();
        assert!(contract::resume(&options, external.path()).is_err());
    }
    let inside = root.path().join("previous.json");
    std::fs::write(&inside, previous.to_string()).unwrap();
    options.resume = Some(inside);
    assert!(contract::resume(&options, external.path()).is_err());
}

#[tokio::test]
async fn invalid_host_options_fail_before_launch_or_evidence_creation() {
    let root = tempfile::tempdir().unwrap();
    let external = tempfile::tempdir().unwrap();
    for case in 0..9 {
        let mut options = options(root.path(), external.path());
        match case {
            0 => options.trust_store = Some(external.path().join("trust")),
            1 => options.max_attempts = 0,
            2 => options.max_attempts = 4,
            3 => options.timeout_seconds = 0,
            4 => options.timeout_seconds = 1801,
            5 => options.role = "../invalid".into(),
            6 => options.output_dir = root.path().into(),
            7 => std::fs::write(&options.agent_command, b"[]").unwrap(),
            8 => std::fs::write(&options.agent_command, b"[\"\"]").unwrap(),
            _ => unreachable!(),
        }
        assert!(run_isolated(&mut options).await.is_err(), "case {case}");
    }
    assert!(!std::fs::read_dir(external.path()).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("agent-loop-")
    }));
}

#[tokio::test]
async fn host_retains_tracked_untracked_and_symlink_edits_and_rejects_bad_refs() {
    let root = super::super::test_support::repository("schema_version: 1\n").await;
    let external = tempfile::tempdir().unwrap();
    let mut options = options(root.path(), external.path());
    let identity = resolve(root.path(), "HEAD").await.unwrap();
    assert_eq!(identity.len(), 40);
    assert!(resolve(root.path(), "--not-a-ref").await.is_err());
    std::fs::write(root.path().join("source.rs"), "fn repaired() {}\n").unwrap();
    std::fs::write(root.path().join("new file.txt"), "new evidence").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink("source.rs", root.path().join("linked")).unwrap();
    let saved = changes(&options, external.path(), "after").await.unwrap();
    assert!(
        std::fs::read_to_string(external.path().join("after.patch"))
            .unwrap()
            .contains("+fn repaired()")
    );
    let untracked = saved["untracked"].as_array().unwrap();
    let file = untracked
        .iter()
        .find(|v| v["path"] == "new file.txt")
        .unwrap();
    assert_eq!(file["kind"], "file");
    assert_eq!(
        file["artifact"]["digest"],
        snapshot::digest(b"new evidence")
    );
    #[cfg(unix)]
    assert_eq!(
        untracked.iter().find(|v| v["path"] == "linked").unwrap()["kind"],
        "symlink"
    );
    options.base = "missing-reference".into();
    assert!(changes(&options, external.path(), "invalid").await.is_err());
    assert!(external.path().join("invalid.patch").exists());
}

#[test]
fn ledger_keeps_transition_history_when_index_advances() {
    let external = tempfile::tempdir().unwrap();
    let mut ledger = json!({"attempts":[],"termination":"running","complete":false});
    persist_ledger(external.path(), &ledger).unwrap();
    ledger["attempts"] = json!([{"status":"running"}]);
    persist_ledger(external.path(), &ledger).unwrap();
    record_execution_error(&mut ledger, &TimeBudgetExpired.into());
    persist_ledger(external.path(), &ledger).unwrap();
    for name in [
        "state-0-initial-running.json",
        "state-1-running-running.json",
        "state-1-budget_exhausted_before_agent-time_budget.json",
    ] {
        assert!(external.path().join(name).is_file(), "{name}");
    }
    let current: Value =
        serde_json::from_slice(&std::fs::read(external.path().join("index.json")).unwrap())
            .unwrap();
    assert_eq!(current, ledger);
}
