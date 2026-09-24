//! Load bounded policy inputs before acquiring repository content.
use super::{CheckOptions, policy};
use crate::{config, domain::PolicyInputError, snapshot};
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
        let bytes = files
            .get(&options.config)
            .ok_or_else(|| PolicyInputError::SnapshotConfigurationMissing {
                path: options.config.clone(),
            })?
            .bytes
            .clone();
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
