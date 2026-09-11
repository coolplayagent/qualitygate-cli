//! Multiset matching preserves moves while distinguishing newly copied entities.

use super::syntax::{self, Entity, Structure};
use crate::snapshot::{self, File, Snapshot};
use anyhow::{Result, bail};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::time::{Duration, Instant};

pub(super) struct TestChange {
    pub entity: Entity,
    pub previous: Option<Entity>,
    pub previous_path: Option<String>,
    pub kind: &'static str,
    pub ambiguous: bool,
}

pub(super) struct FileView {
    pub path: String,
    pub view: Structure,
    pub previous: Option<Structure>,
    pub tests: Vec<TestChange>,
}

pub(super) fn collect(
    snapshot: &Snapshot,
    paths: &[String],
    languages: &[String],
) -> Result<Vec<FileView>> {
    analyze(
        &snapshot.base_files,
        &snapshot.files,
        &snapshot.changes,
        paths,
        languages,
        |path| snapshot.includes(path),
    )
}

pub(super) fn collect_between(
    base: &BTreeMap<String, File>,
    head: &BTreeMap<String, File>,
) -> Result<Vec<FileView>> {
    analyze(
        base,
        head,
        &snapshot::compare_files(base, head),
        &[],
        &[],
        |_| true,
    )
}

fn analyze(
    base_files: &BTreeMap<String, File>,
    files: &BTreeMap<String, File>,
    changes: &BTreeMap<String, snapshot::Change>,
    paths: &[String],
    languages: &[String],
    includes: impl Fn(&str) -> bool,
) -> Result<Vec<FileView>> {
    for language in languages {
        if !["java", "python", "typescript", "go", "rust", "shell"].contains(&language.as_str()) {
            bail!("Syntax capability unavailable for language: {language}");
        }
    }
    let mut filters = globset::GlobSetBuilder::new();
    for path in paths {
        filters.add(globset::Glob::new(path)?);
    }
    let filters = filters.build()?;
    let mut head = BTreeMap::new();
    let mut base = BTreeMap::new();
    let started = Instant::now();
    let mut entity_count = 0;
    // Match across the complete change before filtering; moving into a selected
    // path must not turn an existing entity into a new one.
    for (path, change) in changes {
        if started.elapsed() > Duration::from_secs(30) {
            bail!("Rule structure collection exceeded 30 seconds");
        }
        let language = syntax::language(path);
        if !languages.is_empty()
            && language.is_none_or(|language| !languages.iter().any(|value| value == language))
        {
            continue;
        }
        if let Some(file) = files.get(path)
            && let Some(view) = syntax::parse(path, &file.bytes)?
        {
            entity_count += view.tests.len();
            head.insert(path.clone(), view);
        }
        if let Some(path) = &change.old_path
            && let Some(file) = base_files.get(path)
            && let Some(view) = syntax::parse(path, &file.bytes)?
        {
            entity_count += view.tests.len();
            base.insert(path.clone(), view);
        }
        if entity_count > 50_000 {
            bail!("Rule structure collection exceeded 50000 test entities");
        }
    }
    let old: Vec<_> = base
        .iter()
        .flat_map(|(path, view)| {
            view.tests
                .iter()
                .map(move |test| (path, &view.language, test))
        })
        .collect();
    let mut used = BTreeSet::new();
    let mut matches = BTreeMap::new();
    let mut ambiguous_matches = BTreeSet::new();
    let mut symbols: BTreeMap<_, VecDeque<_>> = BTreeMap::new();
    for (index, (path, language, test)) in old.iter().enumerate() {
        symbols
            .entry((path.as_str(), language.as_str(), test.symbol.as_str()))
            .or_default()
            .push_back(index);
    }
    // Reserve every surviving symbol before assigning a removed body to a move.
    for (path, view) in &head {
        for (index, test) in view.tests.iter().enumerate() {
            if let Some(candidates) =
                symbols.get_mut(&(path.as_str(), view.language.as_str(), test.symbol.as_str()))
            {
                let ambiguous = candidates.len() > 1;
                if let Some(old_index) = candidates.pop_front() {
                    used.insert(old_index);
                    matches.insert((path.clone(), index), old_index);
                    if ambiguous {
                        ambiguous_matches.insert((path.clone(), index));
                    }
                }
            }
        }
    }
    // Preserve named entities across file moves before falling back to body-only
    // matching; repeated trivial bodies otherwise exchange identities on reorder.
    let mut named_bodies: BTreeMap<_, VecDeque<_>> = BTreeMap::new();
    for (index, (_, language, entity)) in old.iter().enumerate() {
        if !used.contains(&index) {
            named_bodies
                .entry((
                    language.to_string(),
                    entity.symbol.clone(),
                    entity.body_digest.clone(),
                ))
                .or_default()
                .push_back(index);
        }
    }
    let mut moved = BTreeMap::new();
    for (path, view) in &head {
        for (index, entity) in view.tests.iter().enumerate() {
            if matches.contains_key(&(path.clone(), index)) {
                continue;
            }
            if let Some(candidates) = named_bodies.get_mut(&(
                view.language.clone(),
                entity.symbol.clone(),
                entity.body_digest.clone(),
            )) {
                let ambiguous = candidates.len() > 1;
                if let Some(previous) = candidates.pop_front() {
                    used.insert(previous);
                    moved.insert((path.clone(), index), (previous, ambiguous));
                }
            }
        }
    }
    let mut bodies: BTreeMap<_, VecDeque<_>> = BTreeMap::new();
    for (index, (_, language, entity)) in old.iter().enumerate() {
        if !used.contains(&index) {
            bodies
                .entry((language.to_string(), entity.body_digest.clone()))
                .or_default()
                .push_back(index);
        }
    }
    let mut result = Vec::new();
    for (path, view) in head {
        let mut tests = Vec::new();
        for (index, entity) in view.tests.iter().enumerate() {
            let direct = matches.get(&(path.clone(), index)).copied();
            let named = moved.get(&(path.clone(), index)).copied();
            let mut ambiguous = named.is_some_and(|(_, ambiguous)| ambiguous)
                || ambiguous_matches.contains(&(path.clone(), index));
            let matched = direct.or(named.map(|(index, _)| index)).or_else(|| {
                let candidates =
                    bodies.get_mut(&(view.language.clone(), entity.body_digest.clone()))?;
                ambiguous = candidates.len() > 1;
                candidates.pop_front()
            });
            let previous = matched.map(|old_index| {
                used.insert(old_index);
                old[old_index].2.clone()
            });
            let kind = match &previous {
                None => "added",
                Some(_) if direct.is_none() => "renamed",
                Some(old)
                    if old.body_digest != entity.body_digest
                        || old.annotations != entity.annotations =>
                {
                    "modified"
                }
                Some(_) => "unchanged",
            };
            tests.push(TestChange {
                entity: entity.clone(),
                previous,
                previous_path: matched.map(|index| old[index].0.clone()),
                kind,
                ambiguous,
            });
        }
        if includes(&path) && (filters.is_empty() || filters.is_match(&path)) {
            let previous = changes[&path]
                .old_path
                .as_ref()
                .and_then(|path| base.get(path))
                .cloned();
            result.push(FileView {
                path,
                view,
                previous,
                tests,
            });
        }
    }
    Ok(result)
}
