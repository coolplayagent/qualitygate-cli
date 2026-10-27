//! Acquire normalized rule subjects, sharing v3 limits across every source.
use super::{
    CustomRule, Range, Result, Snapshot, Subject, file_inventory, require_syntax_capabilities,
};
use crate::{
    adapters::{entity_changes, facts::RuleFacts, markers, syntax},
    domain::operator::{MAX_ENTITIES, MAX_FACT_BYTES},
};
use anyhow::{bail, ensure};
use std::{collections::BTreeMap, time::Instant};

pub(super) struct Budget {
    deadline: Instant,
    entities: usize,
    bytes: usize,
}

impl Budget {
    pub(super) fn new() -> Self {
        Self {
            deadline: crate::adapters::parallel::deadline(),
            entities: 0,
            bytes: 0,
        }
    }

    pub(super) fn deadline(&self) -> Instant {
        self.deadline
    }

    pub(super) fn check(&self) -> Result<()> {
        ensure!(
            Instant::now() < self.deadline,
            "Named source acquisition exceeded its 30-second deadline"
        );
        Ok(())
    }

    // Reserve normalized name/text/path bytes before allocating an owned subject.
    pub(super) fn reserve(&mut self, name: usize, text: usize, path: usize) -> Result<()> {
        self.check()?;
        let bytes = self
            .bytes
            .saturating_add(name)
            .saturating_add(text)
            .saturating_add(path);
        ensure!(
            self.entities < MAX_ENTITIES && bytes <= MAX_FACT_BYTES,
            "Named sources exceed 50000 entities or 32 MiB facts"
        );
        self.entities += 1;
        self.bytes = bytes;
        Ok(())
    }
}

pub(super) fn collect(
    rule: &CustomRule,
    snapshot: &Snapshot,
    facts: &RuleFacts<'_>,
    independent: bool,
    mut budget: Option<&mut Budget>,
) -> Result<Vec<Subject>> {
    if let Some(budget) = &budget {
        budget.check()?;
    }
    let provenance = facts.provenance;
    let change = rule.when.change.as_deref().unwrap_or("added");
    if rule.when.entity == "commit" {
        let mut subjects = Vec::new();
        for (oid, message) in &snapshot.commits {
            let name = message.lines().next().unwrap_or_default();
            if let Some(budget) = budget.as_deref_mut() {
                budget.reserve(name.len(), message.len(), 0)?;
            }
            subjects.push(Subject {
                file: None,
                range: None,
                identity: oid.clone(),
                name: name.into(),
                text: message.clone(),
                declaration: None,
                triggered: true,
            });
        }
        if let Some(budget) = budget {
            budget.check()?;
        }
        return Ok(subjects);
    }
    if rule.when.entity == "file" {
        return file_inventory::selected_subjects(rule, snapshot, independent, budget);
    }
    let files = if let Some(budget) = &budget {
        entity_changes::collect_before(
            snapshot,
            &rule.applies_to.paths,
            &rule.language,
            independent,
            budget.deadline(),
        )?
    } else {
        entity_changes::collect(snapshot, &rule.applies_to.paths, &rule.language)?
    };
    let mut subjects = Vec::new();
    let mut previous_views = BTreeMap::new();
    let started = std::time::Instant::now();
    for file in files {
        if let Some(budget) = &budget {
            budget.check()?;
        }
        if started.elapsed().as_secs() >= 30 {
            bail!("Marker retention exceeds analysis budget");
        }
        require_syntax_capabilities(&file.view.language, &rule.requires_capabilities)?;
        match rule.when.entity.as_str() {
            "test_method" => {
                let bytes = &snapshot.files[&file.path].bytes;
                for test in &file.tests {
                    if let Some(budget) = &budget {
                        budget.check()?;
                    }
                    let in_scope = rule.applies_to.provenance_scope.as_deref() != Some("ai_only")
                        || provenance
                            .is_some_and(|facts| facts.participated(&file.path, &test.entity));
                    let triggered = (change == "any" || test.kind == change) && in_scope;
                    let previous_declaration = if let (Some(binding), Some(previous), Some(path)) =
                        (&rule.binding, &test.previous, &test.previous_path)
                    {
                        if !previous_views.contains_key(path) {
                            let bytes = &snapshot.base_files[path].bytes;
                            let view = if let Some(budget) = &budget {
                                syntax::parse_before(path, bytes, budget.deadline())?
                            } else {
                                syntax::parse(path, bytes)?
                            };
                            previous_views
                                .insert(path.clone(), view.expect("previous supported syntax"));
                        }
                        markers::bound_declaration(
                            &binding.marker,
                            previous,
                            &previous_views[path],
                            path,
                            snapshot,
                            true,
                            facts.git_trailers,
                        )?
                    } else {
                        None
                    };
                    let retained = previous_declaration.is_some();
                    let declaration = rule
                        .binding
                        .as_ref()
                        .map(|binding| {
                            markers::bound_declaration(
                                &binding.marker,
                                &test.entity,
                                &file.view,
                                &file.path,
                                snapshot,
                                false,
                                facts.git_trailers,
                            )
                        })
                        .transpose()?
                        .flatten();
                    if !triggered && !retained && declaration.is_none() {
                        continue;
                    }
                    let entity = &test.entity;
                    let text = &bytes[entity.byte_range.clone()];
                    if let Some(budget) = budget.as_deref_mut() {
                        budget.reserve(entity.name.len(), text.len(), file.path.len())?;
                    }
                    subjects.push(Subject {
                        triggered,
                        file: Some(file.path.clone()),
                        range: if snapshot.delivery()
                            && retained
                            && previous_declaration != declaration
                        {
                            None
                        } else {
                            Some(entity.range.clone())
                        },
                        identity: entity.symbol.clone(),
                        name: entity.name.clone(),
                        text: std::str::from_utf8(text)?.into(),
                        declaration,
                    });
                }
            }
            "comment" | "import" => {
                let mut counts = BTreeMap::new();
                if let Some(previous) = &file.previous {
                    for (text, _) in text_entities(previous, &rule.when.entity) {
                        if let Some(budget) = &budget {
                            budget.check()?;
                        }
                        *counts.entry(text).or_insert(0usize) += 1;
                    }
                }
                for (text, range) in text_entities(&file.view, &rule.when.entity) {
                    if let Some(budget) = &budget {
                        budget.check()?;
                    }
                    if change != "any"
                        && let Some(count) = counts.get_mut(text)
                        && *count > 0
                    {
                        *count -= 1;
                        continue;
                    }
                    if let Some(budget) = budget.as_deref_mut() {
                        budget.reserve(text.len(), text.len(), file.path.len())?;
                    }
                    subjects.push(Subject {
                        triggered: true,
                        file: Some(file.path.clone()),
                        range: Some(range.clone()),
                        identity: text.into(),
                        name: text.into(),
                        text: text.into(),
                        declaration: None,
                    });
                }
            }
            other => bail!("Unsupported DSL entity: {other}"),
        }
    }
    if let Some(budget) = budget {
        budget.check()?;
    }
    Ok(subjects)
}

fn text_entities<'a>(
    view: &'a syntax::Structure,
    kind: &str,
) -> Box<dyn Iterator<Item = (&'a str, &'a Range)> + 'a> {
    if kind == "comment" {
        Box::new(
            view.comments
                .iter()
                .map(|value| (value.text.as_str(), &value.range)),
        )
    } else {
        Box::new(
            view.imports
                .iter()
                .map(|value| (value.text.as_str(), &value.range)),
        )
    }
}

#[cfg(test)]
#[path = "selection_tests.rs"]
mod tests;
