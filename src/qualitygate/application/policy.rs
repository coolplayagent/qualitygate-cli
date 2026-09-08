use super::*;
use crate::{
    config::{Config, Source},
    snapshot::{File, Snapshot},
};
use anyhow::bail;
use std::collections::{BTreeMap, BTreeSet};

pub(super) async fn load(
    snapshot: &Snapshot,
    options: &CheckOptions,
    invalid: &mut Vec<String>,
) -> Result<(Config, PolicyEvidence)> {
    let candidate = &snapshot.files[&options.config];
    let trusted = if let Some(reference) = &options.policy_ref {
        Some(snapshot::read_commit(&snapshot.root, reference).await?)
    } else {
        None
    };
    let selected = trusted.as_ref().unwrap_or(&snapshot.files);
    let file = selected
        .get(&options.config)
        .context("Selected policy reference has no qualitygate configuration")?;
    let config = config::parse(&file.bytes)?;
    let mut changes = Vec::new();
    if trusted.is_some() {
        if file.bytes != candidate.bytes {
            changes.push(options.config.clone());
        }
        if let Some(task) = &options.task
            && selected.get(task).map(|file| &file.bytes)
                != snapshot.files.get(task).map(|file| &file.bytes)
        {
            changes.push(task.clone());
        }
        let mut builder = globset::GlobSetBuilder::new();
        for asset in &config.verification_assets {
            builder.add(globset::Glob::new(asset)?);
        }
        if let Some(custom) = &config.custom_rules {
            builder.add(globset::Glob::new(&format!("{custom}/**"))?);
        }
        let matcher = builder.build()?;
        let paths: BTreeSet<_> = selected.keys().chain(snapshot.files.keys()).collect();
        for path in paths {
            if matcher.is_match(path)
                && selected.get(path).map(|file| &file.bytes)
                    != snapshot.files.get(path).map(|file| &file.bytes)
            {
                changes.push(path.clone());
            }
        }
        for path in &changes {
            invalid.push(format!(
                "Policy/verification asset differs from caller-supplied policy reference: {path}"
            ));
        }
    }
    let policy = PolicyEvidence {
        source: options
            .policy_ref
            .clone()
            .unwrap_or_else(|| options.config.clone()),
        config_digest: snapshot::digest(&file.bytes),
        rules_digest: snapshot::digest(&serde_json::to_vec(&config.rules)?),
        task_contract_digest: None,
        trust: if trusted.is_some() {
            "caller_supplied_ref"
        } else {
            "local_candidate"
        }
        .into(),
        changes,
    };
    Ok((config, policy))
}

pub(super) fn validate_source(source: &Source, files: &BTreeMap<String, File>) -> Result<()> {
    let bytes = &files
        .get(&source.document)
        .with_context(|| format!("Missing source document: {}", source.document))?
        .bytes;
    let text = std::str::from_utf8(bytes)?;
    let mut selected = String::new();
    let mut level = None;
    for line in text.split_inclusive('\n') {
        let title = line.trim_end();
        let hashes = title
            .chars()
            .take_while(|character| *character == '#')
            .count();
        if hashes > 0 && title[hashes..].starts_with(' ') {
            if level.is_some_and(|level| hashes <= level) {
                break;
            }
            if title[hashes..].trim() == source.section {
                level = Some(hashes);
            }
        }
        if level.is_some() {
            selected.push_str(line);
        }
    }
    if level.is_none() {
        bail!("Missing normative section: {}", source.section);
    }
    if snapshot::digest(selected.as_bytes()) != source.content_hash {
        bail!("Normative section digest changed: {}", source.section);
    }
    Ok(())
}
