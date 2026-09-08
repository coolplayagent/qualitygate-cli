use super::*;
use crate::{
    config::{Config, Source, catalog::Catalog},
    snapshot::{File, Snapshot},
};
use anyhow::bail;
use std::collections::{BTreeMap, BTreeSet};

pub(super) async fn load(
    snapshot: &Snapshot,
    options: &CheckOptions,
    invalid: &mut Vec<String>,
) -> Result<(Config, Catalog, PolicyEvidence)> {
    let trusted = if let Some(reference) = &options.policy_ref {
        Some(snapshot::read_commit(&snapshot.root, reference).await?)
    } else {
        None
    };
    let files = snapshot.files.clone();
    let options = options.clone();
    let (config, catalog, policy, errors) = tokio::task::spawn_blocking(move || -> Result<_> {
        let mut invalid = Vec::new();
        let candidate = &files[&options.config];
        let selected = trusted.as_ref().unwrap_or(&files);
        let file = selected
            .get(&options.config)
            .context("Selected policy reference has no qualitygate configuration")?;
        let config = config::parse(&file.bytes)?;
        let catalog = Catalog::load(
            &config,
            selected
                .iter()
                .map(|(path, file)| (path.as_str(), file.bytes.as_slice())),
        )?;
        let config = catalog.resolve(&config)?;
        let mut changes = Vec::new();
        if trusted.is_some() {
            if file.bytes != candidate.bytes {
                changes.push(options.config.clone());
            }
            if let Some(task) = &options.task
                && selected.get(task).map(|file| &file.bytes)
                    != files.get(task).map(|file| &file.bytes)
            {
                changes.push(task.clone());
            }
            let mut builder = globset::GlobSetBuilder::new();
            for asset in &config.verification_assets {
                builder.add(globset::Glob::new(asset)?);
            }
            if let Some(custom) = &config.custom_rules {
                builder.add(globset::Glob::new(&format!(
                    "{}/**",
                    custom.trim_end_matches('/')
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
            rules_digest: snapshot::digest(&serde_json::to_vec(&serde_json::json!({
                "settings": config.rules,
                "definitions": catalog,
                "engine_version": env!("CARGO_PKG_VERSION"),
            }))?),
            task_contract_digest: None,
            trust: if trusted.is_some() {
                "caller_supplied_ref"
            } else {
                "local_candidate"
            }
            .into(),
            changes,
        };
        Ok((config, catalog, policy, invalid))
    })
    .await??;
    invalid.extend(errors);
    Ok((config, catalog, policy))
}

pub(super) fn validate_source(source: &Source, files: &BTreeMap<String, File>) -> Result<()> {
    let bytes = &files
        .get(&source.document)
        .with_context(|| format!("Missing source document: {}", source.document))?
        .bytes;
    let text = std::str::from_utf8(bytes)?;
    let mut headings = Vec::new();
    let mut fence: Option<(char, usize)> = None;
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim();
        if let Some((character, length)) = fence {
            if trimmed
                .chars()
                .take_while(|value| *value == character)
                .count()
                >= length
                && trimmed.trim_matches(character).is_empty()
            {
                fence = None;
            }
            offset += line.len();
            continue;
        }
        let character = trimmed.chars().next().unwrap_or(' ');
        let length = trimmed
            .chars()
            .take_while(|value| *value == character)
            .count();
        if ['`', '~'].contains(&character) && length >= 3 {
            fence = Some((character, length));
            offset += line.len();
            continue;
        }
        let title = line.trim_end();
        let hashes = title
            .chars()
            .take_while(|character| *character == '#')
            .count();
        if (1..=6).contains(&hashes) && title[hashes..].starts_with(' ') {
            headings.push((hashes, title[hashes..].trim(), offset));
        }
        offset += line.len();
    }
    let matching: Vec<_> = headings
        .iter()
        .enumerate()
        .filter(|(_, (_, title, _))| *title == source.section)
        .collect();
    if matching.len() != 1 {
        bail!(
            "Normative section must exist exactly once: {} (found {})",
            source.section,
            matching.len()
        );
    }
    let (index, (level, _, start)) = matching[0];
    let end = headings[index + 1..]
        .iter()
        .find(|(next_level, _, _)| next_level <= level)
        .map_or(text.len(), |(_, _, offset)| *offset);
    let selected = &text[*start..end];
    if snapshot::digest(selected.as_bytes()) != source.content_hash {
        bail!("Normative section digest changed: {}", source.section);
    }
    Ok(())
}

#[cfg(test)]
#[path = "policy_tests.rs"]
mod tests;
