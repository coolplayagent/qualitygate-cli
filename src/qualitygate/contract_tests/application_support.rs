//! Minimal immutable inputs for owner-local unit scenarios; no CLI or project build.
use crate::snapshot::{Identity, Snapshot};
use std::path::Path;

/// Fixture trust paths must not inherit platform aliases such as macOS /var.
pub(crate) fn tempdir() -> tempfile::TempDir {
    tempfile::tempdir_in(dunce::canonicalize(std::env::temp_dir()).unwrap()).unwrap()
}

pub(crate) fn snapshot(root: &Path) -> Snapshot {
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

pub(crate) fn policy() -> crate::domain::PolicyEvidence {
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

pub(crate) fn source(snapshot: &mut Snapshot) -> crate::domain::constraint::Source {
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

pub(crate) fn adoption(binding: &str) -> crate::domain::SourceReview {
    crate::domain::SourceReview {
        id: "review".into(),
        reviewer: "reviewer".into(),
        reference: "review-1".into(),
        resolution: crate::domain::ReviewResolution::InitialMapping,
        rationale: "Matches the declared source".into(),
        binding_digest: binding.into(),
    }
}

pub(crate) async fn repository(policy: &str) -> tempfile::TempDir {
    let root = tempdir();
    crate::snapshot::run_git(root.path(), &["init", "--quiet", "--template="], None)
        .await
        .unwrap();
    crate::snapshot::run_git(root.path(), &["config", "core.autocrlf", "false"], None)
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

pub(crate) fn options(root: &Path) -> super::CheckOptions {
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
