//! Resolve the same policy/catalog/task plan as check before reading project content.

use super::{CheckOptions, acquisition, policy, policy_active};
use crate::{config, snapshot};
use anyhow::{Result, ensure};
use std::sync::Arc;

pub(super) async fn bootstrap(
    options: &mut CheckOptions,
) -> Result<(policy::Loaded, Option<policy_active::Authenticated>)> {
    let active = policy_active::load(options.root.clone()).await?;
    let (_, configuration) =
        acquisition::prepare(options, active.as_ref().map(|active| &active.active.config)).await?;
    let mut bootstrap = options.clone();
    let mut include = vec![globset::escape(&options.config)];
    include.extend(options.task.as_deref().map(globset::escape));
    if let Some(directory) = config::catalog::project_rules_directory(&configuration) {
        include.push(format!("{}/**", globset::escape(directory)));
    }
    bootstrap.snapshot_options.include = Some(include);
    bootstrap.snapshot_options.exclude.clear();
    let snapshot = Arc::new(
        snapshot::capture_with_options(
            &options.root,
            &options.selection,
            &bootstrap.snapshot_options,
        )
        .await?,
    );
    let (loaded, active) = load(&snapshot, &bootstrap, active, false, &mut Vec::new()).await?;
    Ok((loaded, active))
}

pub(super) async fn load(
    snapshot: &Arc<snapshot::Snapshot>,
    options: &CheckOptions,
    active: Option<policy_active::Authenticated>,
    full: bool,
    invalid: &mut Vec<String>,
) -> Result<(policy::Loaded, Option<policy_active::Authenticated>)> {
    if let Some(active) = active {
        ensure!(
            active.active.evaluator_digest() == super::evaluator_digest().await?,
            "Active policy requires revalidation with this evaluator executable"
        );
        let snapshot = Arc::clone(snapshot);
        let options = options.clone();
        let (loaded, errors, active) = tokio::task::spawn_blocking(move || -> Result<_> {
            let mut errors = Vec::new();
            let loaded = if full {
                policy_active::prepare(&active.active, &snapshot, &options, &mut errors)?
            } else {
                policy_active::prepare_plan(&active.active, &snapshot, &options)?
            };
            Ok((loaded, errors, active))
        })
        .await??;
        invalid.extend(errors);
        Ok((loaded, Some(active)))
    } else {
        Ok((
            if full {
                policy::load(snapshot, options, invalid).await?
            } else {
                policy::load_plan(snapshot, options).await?
            },
            None,
        ))
    }
}
