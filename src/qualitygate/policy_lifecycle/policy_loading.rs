use crate::{
    check_planning::{check_request::CheckOptions, snapshot_inputs},
    config::{self, Plan, Source, catalog::Catalog},
    domain::{PolicyEvidence, PolicyInputError},
    snapshot::{self, File, Snapshot},
    snapshot_scope::exclusions,
};
use anyhow::{Context, Result, bail};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

pub(crate) struct Loaded {
    pub catalog: Catalog,
    pub evidence: PolicyEvidence,
    pub plan: Plan,
    pub protected_paths: Vec<String>,
    pub constraint_bindings: BTreeMap<String, String>,
    pub agent_bindings: BTreeMap<String, String>,
}

pub(crate) async fn load(
    snapshot: &Arc<Snapshot>,
    options: &CheckOptions,
    invalid: &mut Vec<String>,
) -> Result<Loaded> {
    load_inner(snapshot, options, invalid, true).await
}

pub(crate) async fn load_plan(snapshot: &Arc<Snapshot>, options: &CheckOptions) -> Result<Loaded> {
    load_inner(snapshot, options, &mut Vec::new(), false).await
}

async fn load_inner(
    snapshot: &Arc<Snapshot>,
    options: &CheckOptions,
    invalid: &mut Vec<String>,
    validate_inputs: bool,
) -> Result<Loaded> {
    let resolved_commit = if let Some(reference) = &options.policy_ref {
        Some(snapshot::resolve_commit(&snapshot.root, reference).await?)
    } else {
        None
    };
    let trusted = if let Some(commit) = &resolved_commit {
        Some(
            snapshot::read_commit_with_options(&snapshot.root, commit, &options.snapshot_options)
                .await?,
        )
    } else {
        None
    };
    let candidate = Arc::clone(snapshot);
    let options = options.clone();
    let (loaded, errors) = tokio::task::spawn_blocking(move || -> Result<_> {
        let files = &candidate.files;
        let mut invalid = Vec::new();
        let selected = trusted.as_ref().unwrap_or(files);
        let file = selected.get(&options.config).ok_or_else(|| {
            PolicyInputError::SnapshotConfigurationMissing {
                path: options.config.clone(),
            }
        })?;
        let config = config::parse(&file.bytes)?;
        let catalog = Catalog::load(
            &config,
            selected
                .iter()
                .map(|(path, file)| (path.as_str(), file.bytes.as_slice())),
        )?;
        let config = catalog.resolve(&config)?;
        let protected_paths =
            snapshot_inputs::resolved(&config, &catalog, &options.config, options.task.as_deref());
        if validate_inputs {
            validate_capture(&config, &candidate, &protected_paths)?;
        }
        let task_file = options
            .task
            .as_ref()
            .map(|path| {
                selected.get(path).with_context(|| {
                    format!("Task contract missing from selected policy snapshot: {path}")
                })
            })
            .transpose()?;
        let task = task_file
            .map(|file| config::parse_task(&file.bytes))
            .transpose()?;
        let plan = Plan::build(&config, task.as_ref(), &options.profile)?;
        let mut changes = BTreeSet::new();
        if trusted.is_some() && validate_inputs {
            if Some((&file.bytes, file.executable))
                != files
                    .get(&options.config)
                    .map(|file| (&file.bytes, file.executable))
            {
                changes.insert(options.config.clone());
            }
            if let Some(task) = &options.task
                && selected
                    .get(task)
                    .map(|file| (&file.bytes, file.executable))
                    != files.get(task).map(|file| (&file.bytes, file.executable))
            {
                changes.insert(task.clone());
            }
            let mut builder = globset::GlobSetBuilder::new();
            for asset in &config.verification_assets {
                builder.add(globset::Glob::new(asset)?);
            }
            if let Some(project_rules) = crate::config::catalog::project_rules_directory(&config) {
                builder.add(globset::Glob::new(&format!(
                    "{}/**",
                    project_rules.trim_end_matches('/')
                ))?);
            }
            let matcher = builder.build()?;
            let paths: BTreeSet<_> = selected.keys().chain(files.keys()).collect();
            for path in paths {
                if matcher.is_match(path)
                    && selected
                        .get(path)
                        .map(|file| (&file.bytes, file.executable))
                        != files.get(path).map(|file| (&file.bytes, file.executable))
                {
                    changes.insert(path.clone());
                }
            }
            for path in &changes {
                invalid.push(format!(
                "Policy/verification asset differs from caller-supplied policy reference: {path}"
            ));
            }
        }
        let policy = PolicyEvidence {
            resolved_commit,
            source_reviews: config::source_reviews::evidence(&config, &catalog)?,
            source: options
                .policy_ref
                .clone()
                .unwrap_or_else(|| options.config.clone()),
            config_digest: snapshot::digest(&file.bytes),
            rules_digest: config::policy_promotion::rules_digest(&config, &catalog)?,
            task_contract_digest: task_file.map(|file| snapshot::digest(&file.bytes)),
            task_contract_source: options.task.clone(),
            trust: if trusted.is_some() {
                "caller_supplied_ref"
            } else {
                "local_candidate"
            }
            .into(),
            changes: changes.into_iter().collect(),
        };
        Ok((
            Loaded {
                constraint_bindings: config::constraint_inventory::bindings(&config, &catalog)?,
                agent_bindings: config::agent_contract::bindings(&config, &catalog)?,
                protected_paths,
                catalog,
                evidence: policy,
                plan,
            },
            invalid,
        ))
    })
    .await??;
    invalid.extend(errors);
    Ok(loaded)
}

pub(crate) fn validate_source(source: &Source, files: &BTreeMap<String, File>) -> Result<()> {
    let bytes = &files
        .get(&source.document)
        .with_context(|| format!("Missing source document: {}", source.document))?
        .bytes;
    let text = std::str::from_utf8(bytes)?;
    let selected = crate::domain::normative::section(text, &source.section)?;
    if snapshot::digest(selected.as_bytes()) != source.content_hash {
        bail!("Normative section digest changed: {}", source.section);
    }
    Ok(())
}

#[cfg(test)]
#[path = "policy_loading_tests.rs"]
mod tests;

pub(crate) fn validate_capture(
    config: &config::Config,
    snapshot: &Snapshot,
    protected_paths: &[String],
) -> Result<()> {
    let captured = &snapshot.scope_evidence;
    exclusions::validate_capture(
        &config.exclude,
        &config.unstructured_documents,
        protected_paths,
        &captured.exclude,
        &captured.excluded_paths,
        &captured.resource_exclusions,
    )
}
