//! Load bounded policy inputs before acquiring repository content.
use super::{CheckOptions, policy};
use crate::domain::prerequisites::{FailureCode, Phase, PrerequisiteIssue};
use crate::{config, snapshot};
use anyhow::{Result, ensure};

pub(super) async fn prepare(
    options: &mut CheckOptions,
    active: Option<&config::Config>,
) -> Result<Option<crate::domain::MergeRequest>> {
    let mut comparison = None;
    let mut bootstrap = options.snapshot_options.clone();
    bootstrap.include = Some(vec![globset::escape(&options.config)]);
    bootstrap.exclude.clear();
    let config = if let Some(config) = active {
        config.clone()
    } else {
        let files = if let Some(reference) = &options.policy_ref {
            snapshot::read_commit_with_options(&options.root, reference, &bootstrap).await?
        } else {
            let captured =
                snapshot::capture_with_options(&options.root, &options.selection, &bootstrap)
                    .await?;
            comparison = captured.identity.merge_request;
            captured.files
        };
        let bytes = match files.get(&options.config) {
            Some(file) => file.bytes.clone(),
            None => return Err(missing_configuration(options).await),
        };
        tokio::task::spawn_blocking(move || config::parse(&bytes)).await??
    };
    let matcher = config::exclusions::matcher(&config.exclude)?;
    ensure!(
        !matcher.is_match(&options.config),
        "exclude matches the selected configuration"
    );
    if let Some(task) = &options.task {
        ensure!(
            !matcher.is_match(task),
            "exclude matches the task contract: {task}"
        );
    }
    options.snapshot_options.exclude = config.exclude;
    options.snapshot_options.include = None;
    Ok(comparison)
}

async fn missing_configuration(options: &CheckOptions) -> anyhow::Error {
    let local = options.policy_ref.is_none()
        && matches!(
            options.selection,
            snapshot::Selection::Worktree { .. }
                | snapshot::Selection::Path { .. }
                | snapshot::Selection::Staged
        );
    if local {
        let (root, path) = (options.root.clone(), options.config.clone());
        match tokio::task::spawn_blocking(move || {
            config::read_candidate_bytes(&root, path.as_ref())
        })
        .await
        {
            Ok(Err(error)) => return error,
            Err(error) => return error.into(),
            Ok(Ok(_)) => {}
        }
    }
    let instruction = if options.policy_ref.is_none()
        && matches!(options.selection, snapshot::Selection::Staged)
    {
        "Review the existing configuration and stage it before repeating the staged check."
    } else if local {
        "Ensure the configuration is included in the selected worktree snapshot; review ignore rules and the selected root."
    } else {
        "Select a Git/policy reference containing the configuration. Initializing the worktree does not alter historical snapshots."
    };
    PrerequisiteIssue::new(
        FailureCode::PolicySnapshotMissing,
        Phase::Policy,
        "Selected policy snapshot has no qualitygate configuration",
    )
    .resource(options.config.clone())
    .instruction(instruction)
    .into()
}

pub(super) fn validate_excluded(snapshot: &snapshot::Snapshot, protected: &[String]) -> Result<()> {
    let mut builder = globset::GlobSetBuilder::new();
    for pattern in protected {
        builder.add(globset::Glob::new(pattern)?);
    }
    let matcher = builder.build()?;
    for path in &snapshot.scope_evidence.excluded_paths {
        ensure!(
            !matcher.is_match(path),
            "exclude matches a protected verification input: {path}"
        );
    }
    Ok(())
}

pub(super) fn validate_config(
    config: &config::Config,
    catalog: &config::catalog::Catalog,
    options: &CheckOptions,
    snapshot: &snapshot::Snapshot,
) -> Result<()> {
    ensure!(
        config.exclude == snapshot.scope_evidence.exclude,
        "Exclusion policy changed during acquisition"
    );
    validate_excluded(
        snapshot,
        &policy::protected_paths(config, catalog, &options.config, options.task.as_deref()),
    )
}
