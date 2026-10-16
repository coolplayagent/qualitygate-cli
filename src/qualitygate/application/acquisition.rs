//! Load bounded policy inputs before acquiring repository content.
use super::{CheckOptions, policy};
use crate::{config, domain::PolicyInputError, snapshot};
use anyhow::{Result, ensure};

/// Init's advisory metadata view uses the same resolved source protection as check.
pub async fn initialization_preflight(
    root: &std::path::Path,
    config: &config::Config,
    configuration: &str,
) -> Result<crate::domain::snapshot_budget::Preflight> {
    let mut inputs = std::collections::BTreeMap::new();
    if let Some(directory) = config::catalog::project_rules_directory(config) {
        inputs = snapshot::capture_with_options(
            root,
            &snapshot::Selection::Worktree {
                base: "HEAD".into(),
            },
            &snapshot::CaptureOptions {
                include: Some(vec![format!("{}/**", globset::escape(directory))]),
                ..Default::default()
            },
        )
        .await?
        .files;
    }
    let config = config.clone();
    let configuration = configuration.to_owned();
    let options = tokio::task::spawn_blocking(move || -> Result<_> {
        let catalog = config::catalog::Catalog::load(
            &config,
            inputs
                .iter()
                .map(|(path, file)| (path.as_str(), file.bytes.as_slice())),
        )?;
        let config = catalog.resolve(&config)?;
        Ok(snapshot::CaptureOptions {
            protected_paths: policy::protected_paths(&config, &catalog, &configuration, None),
            exclude: config.exclude,
            unstructured_documents: config.unstructured_documents,
            ..Default::default()
        })
    })
    .await??;
    snapshot::preflight::inspect_options(root, &options).await
}

pub(super) async fn prepare(
    options: &mut CheckOptions,
    active: Option<&config::policy_active::Active>,
) -> Result<(Option<crate::domain::MergeRequest>, config::Config)> {
    let mut comparison = None;
    let mut bootstrap = options.snapshot_options.clone();
    bootstrap.include = Some(vec![globset::escape(&options.config)]);
    bootstrap.exclude.clear();
    let config = if let Some(config) = active {
        config.config.clone()
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
    // Resolve custom rule sources before applying default resource exclusions.
    // These bounded inputs cannot be filtered by suffix during bootstrap.
    let catalog = if let Some(active) = active {
        let frozen = active.frozen.clone();
        let configuration = config.clone();
        tokio::task::spawn_blocking(move || {
            frozen.resolve(&configuration).map(|(_, catalog)| catalog)
        })
        .await??
    } else {
        let mut inputs = std::collections::BTreeMap::new();
        if let Some(directory) = config::catalog::project_rules_directory(&config) {
            bootstrap.include = Some(vec![format!("{}/**", globset::escape(directory))]);
            inputs = if let Some(reference) = &options.policy_ref {
                snapshot::read_commit_with_options(&options.root, reference, &bootstrap).await?
            } else {
                snapshot::capture_resolved(
                    &options.root,
                    &options.selection,
                    &bootstrap,
                    comparison.clone(),
                )
                .await?
                .files
            };
        }
        let configuration = config.clone();
        tokio::task::spawn_blocking(move || {
            config::catalog::Catalog::load(
                &configuration,
                inputs
                    .iter()
                    .map(|(path, file)| (path.as_str(), file.bytes.as_slice())),
            )
        })
        .await??
    };
    let (config, catalog) = tokio::task::spawn_blocking(move || {
        Ok::<_, anyhow::Error>((catalog.resolve(&config)?, catalog))
    })
    .await??;
    options.snapshot_options.protected_paths =
        policy::protected_paths(&config, &catalog, &options.config, options.task.as_deref());
    options.snapshot_options.unstructured_documents = config.unstructured_documents.clone();
    options.snapshot_options.exclude = config.exclude.clone();
    options.snapshot_options.include = None;
    Ok((comparison, config))
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
        config.exclude == snapshot.scope_evidence.exclude
            && config.unstructured_documents == snapshot.scope_evidence.resource_exclusions.policy
            && policy::protected_paths(config, catalog, &options.config, options.task.as_deref())
                == snapshot.scope_evidence.resource_exclusions.protected_paths,
        "Exclusion policy changed during acquisition"
    );
    validate_excluded(
        snapshot,
        &policy::protected_paths(config, catalog, &options.config, options.task.as_deref()),
    )
}
