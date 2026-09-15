use super::*;
use crate::{
    config::{Source, catalog::Catalog},
    snapshot::{File, Snapshot},
};
use anyhow::bail;
use std::collections::{BTreeMap, BTreeSet};

pub(super) struct Loaded {
    pub catalog: Catalog,
    pub evidence: PolicyEvidence,
    pub plan: Plan,
    pub protected_paths: Vec<String>,
}

pub(super) async fn load(
    snapshot: &Arc<Snapshot>,
    options: &CheckOptions,
    invalid: &mut Vec<String>,
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
        let file = selected
            .get(&options.config)
            .context("Selected policy snapshot has no qualitygate configuration; run init and stage/commit it as appropriate")?;
        let config = config::parse(&file.bytes)?;
        let catalog = Catalog::load(
            &config,
            selected
                .iter()
                .map(|(path, file)| (path.as_str(), file.bytes.as_slice())),
        )?;
        let config = catalog.resolve(&config)?;
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
        if trusted.is_some() {
            if Some((&file.bytes, file.executable))
                != files.get(&options.config).map(|file| (&file.bytes, file.executable))
            {
                changes.insert(options.config.clone());
            }
            if let Some(task) = &options.task
                && selected.get(task).map(|file| (&file.bytes, file.executable))
                    != files.get(task).map(|file| (&file.bytes, file.executable))
            {
                changes.insert(task.clone());
            }
            let mut builder = globset::GlobSetBuilder::new();
            for asset in &config.verification_assets {
                builder.add(globset::Glob::new(asset)?);
            }
            if let Some(project_rules) = crate::config::catalog::project_rules_directory(&config)
            {
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
            rules_digest: snapshot::digest(&serde_json::to_vec(&serde_json::json!({
                "settings": config.rules,
                "definitions": catalog,
                "source_reviews": config.source_reviews,
                "engine_version": env!("CARGO_PKG_VERSION"),
            }))?),
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
        Ok((Loaded { protected_paths: protected_paths(&config, &catalog, &options.config, options.task.as_deref()), catalog, evidence: policy, plan }, invalid))
    })
    .await??;
    invalid.extend(errors);
    Ok(loaded)
}

pub(super) fn validate_source(source: &Source, files: &BTreeMap<String, File>) -> Result<()> {
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
#[path = "policy_tests.rs"]
mod tests;

pub(super) fn protected_paths(
    config: &config::Config,
    catalog: &Catalog,
    configuration: &str,
    task: Option<&str>,
) -> Vec<String> {
    let mut paths = config.verification_assets.clone();
    paths.push(globset::escape(configuration));
    paths.extend(task.map(globset::escape));
    if let Some(directory) = config::catalog::project_rules_directory(config) {
        paths.push(format!("{}/**", globset::escape(directory)));
    }
    for source in config
        .rules
        .values()
        .filter_map(|rule| rule.source.as_ref())
        .chain(
            catalog
                .entries
                .values()
                .filter_map(|entry| entry.custom.as_ref().map(|rule| &rule.source)),
        )
    {
        paths.push(globset::escape(&source.document));
    }
    paths
}
