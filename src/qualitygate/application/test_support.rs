//! Minimal immutable inputs for owner-local unit scenarios; no CLI or project build.
use crate::snapshot::{Identity, Snapshot};
use std::path::Path;

pub(super) fn snapshot(root: &Path) -> Snapshot {
    Snapshot {
        root: root.into(),
        scope_evidence: Default::default(),
        identity: Identity {
            verification_digest: None,
            merge_request: None,
            mode: "worktree".into(),
            base: "base".into(),
            head: "head".into(),
            content_digest: crate::snapshot::digest(b"test snapshot"),
        },
        files: Default::default(),
        base_files: Default::default(),
        changes: Default::default(),
        path_filter: None,
        commits: vec![],
    }
}

pub(super) fn policy() -> crate::domain::PolicyEvidence {
    crate::domain::PolicyEvidence {
        source: "unit-policy".into(),
        resolved_commit: None,
        config_digest: crate::snapshot::digest(b"policy"),
        rules_digest: crate::snapshot::digest(b"rules"),
        task_contract_digest: None,
        task_contract_source: None,
        trust: "local_candidate".into(),
        changes: vec![],
        source_reviews: Default::default(),
    }
}

pub(super) fn source(snapshot: &mut Snapshot) -> crate::domain::constraint::Source {
    let text = b"# Rules\nKeep meaningful tests.\n";
    snapshot.files.insert(
        "AGENTS.md".into(),
        crate::snapshot::File {
            bytes: text.to_vec(),
            executable: false,
        },
    );
    crate::domain::constraint::Source {
        document: "AGENTS.md".into(),
        section: "Rules".into(),
        content_hash: crate::snapshot::digest(text),
    }
}

pub(super) fn adoption(binding: &str) -> crate::domain::SourceReview {
    crate::domain::SourceReview {
        id: "review".into(),
        reviewer: "reviewer".into(),
        reference: "review-1".into(),
        resolution: crate::domain::ReviewResolution::InitialMapping,
        rationale: "Matches the declared source".into(),
        binding_digest: binding.into(),
    }
}

pub(super) async fn repository(policy: &str) -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    crate::snapshot::run_git(root.path(), &["init", "--quiet", "--template="], None)
        .await
        .unwrap();
    std::fs::write(root.path().join("qualitygate.yaml"), policy).unwrap();
    std::fs::write(root.path().join("source.rs"), "fn example() {}\n").unwrap();
    crate::snapshot::run_git(
        root.path(),
        &["-c", "core.autocrlf=false", "add", "."],
        None,
    )
    .await
    .unwrap();
    crate::snapshot::run_git(
        root.path(),
        &[
            "-c",
            "user.name=Unit Test",
            "-c",
            "user.email=unit@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--quiet",
            "-m",
            "test: initial snapshot",
        ],
        None,
    )
    .await
    .unwrap();
    root
}

pub(super) fn options(root: &Path) -> super::CheckOptions {
    super::CheckOptions {
        root: root.into(),
        config: "qualitygate.yaml".into(),
        selection: crate::snapshot::Selection::Worktree {
            base: "HEAD".into(),
        },
        snapshot_options: crate::snapshot::CaptureOptions {
            scope: crate::domain::check_scope::CheckScope::Delivery,
            ..Default::default()
        },
        profile: "full".into(),
        task: None,
        policy_ref: None,
        output_dir: None,
        trust_store: None,
        evidence_dir: None,
    }
}
