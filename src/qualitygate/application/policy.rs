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
}

pub(super) async fn load(
    snapshot: &Snapshot,
    options: &CheckOptions,
    invalid: &mut Vec<String>,
) -> Result<Loaded> {
    let resolved_commit = if let Some(reference) = &options.policy_ref {
        Some(snapshot::resolve_commit(&snapshot.root, reference).await?)
    } else {
        None
    };
    let trusted = if let Some(commit) = &resolved_commit {
        Some(snapshot::read_commit(&snapshot.root, commit).await?)
    } else {
        None
    };
    let files = snapshot.files.clone();
    let options = options.clone();
    let (loaded, errors) = tokio::task::spawn_blocking(move || -> Result<_> {
        let mut invalid = Vec::new();
        let selected = trusted.as_ref().unwrap_or(&files);
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
        Ok((Loaded { catalog, evidence: policy, plan }, invalid))
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
    let mut headings = Vec::new();
    let mut depth = 0usize;
    for (event, range) in pulldown_cmark::Parser::new(text).into_offset_iter() {
        match event {
            pulldown_cmark::Event::Start(tag) => {
                if depth == 0
                    && let pulldown_cmark::Tag::Heading { level, .. } = tag
                {
                    let line = text[range.start..].lines().next().unwrap_or_default();
                    let hashes = line
                        .chars()
                        .take_while(|character| *character == '#')
                        .count();
                    // Preserve the documented raw ATX title and exact section
                    // bytes; CommonMark determines whether this is a heading.
                    if (1..=6).contains(&hashes) && line[hashes..].starts_with(' ') {
                        headings.push((level as usize, line[hashes..].trim(), range.start));
                    }
                }
                depth += 1;
            }
            pulldown_cmark::Event::End(_) => depth -= 1,
            _ => {}
        }
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
