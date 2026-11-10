use super::*;

#[tokio::test]
async fn missing_policy_guidance_preserves_selector_without_staging_or_overwriting_files() {
    let root = tempfile::tempdir().unwrap();
    for exists in [false, true] {
        if exists {
            std::fs::write(root.path().join("custom.yaml"), b"malformed but present").unwrap();
        }
        for selection in [
            SelectionKind::Worktree,
            SelectionKind::Staged,
            SelectionKind::Diff,
            SelectionKind::MergeRequest,
            SelectionKind::Path,
        ] {
            let context = Context {
                root: root.path().into(),
                config: "custom.yaml".into(),
                selection: Some(selection),
                policy_ref: false,
            };
            let error = anyhow::Error::new(PolicyInputError::SnapshotConfigurationMissing {
                path: "custom.yaml".into(),
            })
            .context("selected snapshot");
            let steps = next_steps(&error, context).await.unwrap();
            assert_eq!(
                steps
                    .iter()
                    .any(|step| step.action == "initialize_candidate_policy"),
                !exists
            );
            assert_eq!(
                steps
                    .iter()
                    .any(|step| step.action == "include_policy_in_snapshot"),
                exists || !matches!(selection, SelectionKind::Worktree)
            );
            for step in &steps {
                if let Some(command) = &step.command {
                    assert!(
                        command
                            .windows(2)
                            .any(|pair| pair == ["--config", "custom.yaml"])
                    );
                    assert!(command.iter().any(|arg| arg == "init"));
                    assert!(!command.iter().any(|arg| arg == "add" || arg == "commit"));
                }
            }
            assert_eq!(root.path().join("custom.yaml").exists(), exists);
        }
    }
    assert_eq!(
        std::fs::read(root.path().join("custom.yaml")).unwrap(),
        b"malformed but present"
    );
}

#[tokio::test]
async fn explicit_policy_reference_is_never_replaced_by_local_initialization() {
    let root = tempfile::tempdir().unwrap();
    let context = Context {
        root: root.path().into(),
        config: "qualitygate.yaml".into(),
        selection: Some(SelectionKind::Diff),
        policy_ref: true,
    };
    let error = PolicyInputError::SnapshotConfigurationMissing {
        path: "qualitygate.yaml".into(),
    }
    .into();
    let steps = next_steps(&error, context).await.unwrap();
    assert_eq!(steps.len(), 1);
    assert_eq!(steps[0].action, "select_policy_reference");
    assert!(steps[0].command.is_none());
    assert!(!root.path().join("qualitygate.yaml").exists());
}

#[tokio::test]
async fn unknown_failures_unconfined_paths_and_non_files_do_not_generate_unsafe_repairs() {
    let root = tempfile::tempdir().unwrap();
    let mut context = Context {
        root: root.path().into(),
        config: "qualitygate.yaml".into(),
        selection: None,
        policy_ref: false,
    };
    assert!(
        next_steps(&anyhow::anyhow!("unrelated failure"), context.clone())
            .await
            .unwrap()
            .is_empty()
    );
    let missing = PolicyInputError::SnapshotConfigurationMissing {
        path: "qualitygate.yaml".into(),
    }
    .into();
    assert!(
        next_steps(&missing, context.clone())
            .await
            .unwrap()
            .is_empty()
    );
    context.selection = Some(SelectionKind::Worktree);
    context.config = "../outside.yaml".into();
    assert!(
        next_steps(&missing, context.clone())
            .await
            .unwrap()
            .is_empty()
    );
    context.config = "directory".into();
    std::fs::create_dir(root.path().join("directory")).unwrap();
    assert!(
        next_steps(&missing, context.clone())
            .await
            .unwrap()
            .is_empty()
    );
    context.root = root.path().join("absent");
    assert!(next_steps(&missing, context).await.unwrap().is_empty());
}
