//! Finite DSL version 1 over snapshot, Git and language facts.

use super::{entity_changes, markers, rules::diagnostic, syntax};
use crate::{
    config::{CustomRule, RuleSetting},
    domain::*,
    snapshot::Snapshot,
};
use anyhow::{Result, bail};
use regex::Regex;
use std::collections::BTreeMap;

struct Subject {
    file: Option<String>,
    range: Option<Range>,
    identity: String,
    name: String,
    text: String,
    declaration: Option<String>,
    triggered: bool,
}

fn require_syntax_capabilities(language: &str, capabilities: &[String]) -> Result<()> {
    let descriptor = crate::domain::language::named(language)
        .ok_or_else(|| anyhow::anyhow!("Syntax capability unavailable for language: {language}"))?;
    for capability in capabilities {
        if ["test_methods", "annotations", "comments", "imports"].contains(&capability.as_str())
            && !descriptor
                .syntax_capabilities
                .contains(&capability.as_str())
        {
            bail!("{capability} capability unavailable for {language}");
        }
    }
    Ok(())
}

pub fn evaluate(rule: &CustomRule, setting: &RuleSetting, snapshot: &Snapshot) -> CheckResult {
    evaluate_with_projects(rule, setting, snapshot, &[])
}

pub fn evaluate_with_projects(
    rule: &CustomRule,
    setting: &RuleSetting,
    snapshot: &Snapshot,
    projects: &[ProjectFacts],
) -> CheckResult {
    evaluate_with_facts(rule, setting, snapshot, projects, None)
}

pub fn evaluate_with_facts(
    rule: &CustomRule,
    setting: &RuleSetting,
    snapshot: &Snapshot,
    projects: &[ProjectFacts],
    provenance: Option<&super::provenance::ProvenanceFacts>,
) -> CheckResult {
    let mut result = CheckResult::pending(&rule.id, setting.required, setting.severity);
    result.rule_version = rule.version;
    let evaluation = (|| -> Result<()> {
        if let Some(facts) = provenance {
            facts.ensure_binding(snapshot, &rule.id)?;
        }
        if setting.provenance.is_some() && provenance.is_none() {
            bail!("Configured provenance evidence is unavailable");
        }
        run(rule, snapshot, projects, provenance, &mut result)
    })();
    match evaluation {
        Ok(()) => result.complete(),
        Err(error) => result.block(ExecutionStatus::Blocked, format!("{error:#}")),
    }
    result
}

fn run(
    rule: &CustomRule,
    snapshot: &Snapshot,
    projects: &[ProjectFacts],
    provenance: Option<&super::provenance::ProvenanceFacts>,
    result: &mut CheckResult,
) -> Result<()> {
    for language in &rule.language {
        require_syntax_capabilities(language, &rule.requires_capabilities)?;
    }
    if let Some(capability) = rule
        .requires_capabilities
        .iter()
        .find(|capability| capability.as_str() == "external_provenance")
        && provenance.is_none()
    {
        bail!("Required project capability unavailable: {capability}");
    }
    if rule.applies_to.provenance_scope.as_deref() == Some("ai_only") && provenance.is_none() {
        bail!("AI-only scope requires verified external execution provenance");
    }
    if rule
        .binding
        .as_ref()
        .is_some_and(|binding| binding.marker.kind == "git_trailer")
    {
        bail!("Commit-to-entity provenance is required for git_trailer binding");
    }
    if rule
        .requires_capabilities
        .iter()
        .any(|capability| capability == "dependency_resolution")
        && projects.is_empty()
    {
        bail!(
            "Required project capability unavailable: dependency_resolution; declare depends_on for a project facts command"
        );
    }
    let subjects = subjects(rule, snapshot, provenance)?;
    let triggered = subjects.iter().filter(|subject| subject.triggered).count();
    result.matched_entities = subjects.len();
    result.metadata.insert(
        "retained_marker_entities".into(),
        serde_json::json!(subjects.len() - triggered),
    );
    result
        .metadata
        .insert("increment_mode".into(), serde_json::json!("entity_changes"));
    result.metadata.insert(
        "change".into(),
        serde_json::json!(rule.when.change.as_deref().unwrap_or("added")),
    );
    result.metadata.insert(
        "requires_capabilities".into(),
        serde_json::json!(rule.requires_capabilities),
    );
    let name = rule
        .then
        .name_pattern
        .as_deref()
        .map(Regex::new)
        .transpose()?;
    let forbid = rule
        .then
        .forbid_pattern
        .as_deref()
        .map(Regex::new)
        .transpose()?;
    for subject in &subjects {
        let mut violations = Vec::new();
        if subject.triggered
            && name
                .as_ref()
                .is_some_and(|pattern| !pattern.is_match(&subject.name))
        {
            violations.push((
                "name_pattern",
                format!(
                    "{} does not match the configured name pattern",
                    subject.name
                ),
            ));
        }
        if subject.triggered
            && forbid
                .as_ref()
                .is_some_and(|pattern| pattern.is_match(&subject.text))
        {
            violations.push((
                "forbid_pattern",
                "Entity contains text forbidden by this rule".into(),
            ));
        }
        if rule.then.require_marker {
            let marker = &rule
                .binding
                .as_ref()
                .expect("validated marker binding")
                .marker;
            let values = subject
                .declaration
                .as_deref()
                .map(markers::fields)
                .unwrap_or_default();
            let missing: Vec<_> = marker
                .fields
                .iter()
                .filter(|field| {
                    values
                        .get(*field)
                        .is_none_or(|value| value.trim().is_empty())
                })
                .collect();
            if subject.declaration.is_none() || !missing.is_empty() {
                violations.push((
                    "require_marker",
                    format!(
                        "Missing {} declaration or nonempty fields: {}",
                        marker.name,
                        missing.into_iter().cloned().collect::<Vec<_>>().join(", ")
                    ),
                ));
            }
        }
        for (kind, message) in violations {
            result.diagnostics.push(diagnostic(&rule.id, subject.file.as_deref(), subject.range.clone(), message,
                serde_json::json!({"symbol":subject.identity,"assertion":kind,"entity":rule.when.entity,"declaration_only":rule.then.require_marker}),
                &rule.fix, &format!("{}:{kind}", subject.identity)));
        }
    }
    if rule.then.require_dependency.is_some() {
        dependencies(rule, snapshot, projects, &subjects, result)?;
    }
    if let Some(maximum) = rule.then.max_count
        && triggered > maximum
    {
        result.diagnostics.push(diagnostic(
            &rule.id,
            None,
            None,
            format!(
                "{} matching entities exceeds max_count {maximum}",
                triggered
            ),
            serde_json::json!({"count":triggered,"maximum":maximum,"entity":rule.when.entity}),
            &rule.fix,
            "max_count",
        ));
    }
    Ok(())
}

fn dependencies(
    rule: &CustomRule,
    snapshot: &Snapshot,
    projects: &[ProjectFacts],
    subjects: &[Subject],
    result: &mut CheckResult,
) -> Result<()> {
    let required = rule
        .then
        .require_dependency
        .as_ref()
        .expect("dependency assertion");
    if projects.iter().any(|project| {
        project.schema_version != 1
            || !["maven", "python"].contains(&project.ecosystem.as_str())
            || project.snapshot_digest != snapshot.identity.content_digest
    }) {
        bail!("Project facts are unsupported or belong to another snapshot");
    }
    let mut selected: BTreeMap<_, _> = subjects
        .iter()
        .filter_map(|subject| {
            subject.file.as_ref().map(|path| {
                (
                    (path.clone(), subject.identity.clone()),
                    subject.range.clone(),
                )
            })
        })
        .collect();
    let mut filters = globset::GlobSetBuilder::new();
    for pattern in &rule.applies_to.paths {
        filters.add(globset::Glob::new(pattern)?);
    }
    let filters = filters.build()?;
    let started = std::time::Instant::now();
    if let Some(binding) = &rule.binding {
        // An unchanged marked test still needs its dependency after a manifest-only edit.
        for (path, file) in &snapshot.files {
            if !snapshot.includes(path) || (!filters.is_empty() && !filters.is_match(path)) {
                continue;
            }
            if !rule.language.is_empty()
                && syntax::language(path)
                    .is_none_or(|language| !rule.language.iter().any(|value| value == language))
            {
                continue;
            }
            if started.elapsed().as_secs() >= 30 || selected.len() > 50_000 {
                bail!("Dependency scope exceeds analysis budget");
            }
            if let Some(view) = syntax::parse(path, &file.bytes)? {
                for test in &view.tests {
                    if markers::from_source(&binding.marker, test, &view, &file.bytes)?.is_some() {
                        selected.insert(
                            (path.clone(), test.symbol.clone()),
                            Some(test.range.clone()),
                        );
                    }
                }
            }
        }
    }
    result.metadata.insert("dependency_scope".into(), serde_json::json!({"mode":"selected_and_retained_marked_tests", "tests":selected.len(), "producers":projects.iter().map(|project| &project.producer_check).collect::<Vec<_>>()}));
    for ((path, symbol), range) in selected {
        if started.elapsed().as_secs() >= 30 {
            bail!("Dependency scope exceeds analysis budget");
        }
        let language = syntax::language(&path);
        let owners: Vec<_> = projects
            .iter()
            .filter(|project| {
                project.owns_test(&path)
                    && matches!(
                        (project.ecosystem.as_str(), language),
                        ("maven", Some("java")) | ("python", Some("python"))
                    )
            })
            .collect();
        if owners.len() != 1 {
            bail!(
                "Expected one resolved test module for {path}, found {}",
                owners.len()
            );
        }
        let project = owners[0];
        let (group, artifact) = if project.ecosystem == "python" {
            if required
                .group
                .as_deref()
                .is_some_and(|group| group != "pypi")
            {
                bail!("Python distribution assertions use the pypi group or omit group");
            }
            (
                "pypi",
                required
                    .artifact
                    .parse::<pep508_rs::PackageName>()?
                    .to_string(),
            )
        } else {
            (
                required
                    .group
                    .as_deref()
                    .filter(|group| !group.trim().is_empty())
                    .ok_or_else(|| {
                        anyhow::anyhow!("Maven dependency assertions require group and artifact")
                    })?,
                required.artifact.clone(),
            )
        };
        if !project.has_test_dependency(group, &artifact) {
            result.diagnostics.push(diagnostic(&rule.id, Some(&path), range,
                format!("Required test dependency {group}:{artifact} is not declared and resolved in the test environment of {}", project.coordinate),
                serde_json::json!({"assertion":"require_dependency", "symbol":symbol, "project":project.coordinate, "manifest":project.manifest, "producer":project.producer_check, "group":group,"artifact":required.artifact}),
                &rule.fix, &format!("{symbol}:require_dependency")));
        }
    }
    Ok(())
}

fn subjects(
    rule: &CustomRule,
    snapshot: &Snapshot,
    provenance: Option<&super::provenance::ProvenanceFacts>,
) -> Result<Vec<Subject>> {
    let change = rule.when.change.as_deref().unwrap_or("added");
    if rule.when.entity == "commit" {
        return Ok(snapshot
            .commits
            .iter()
            .map(|(oid, message)| Subject {
                file: None,
                range: None,
                identity: oid.clone(),
                name: message.lines().next().unwrap_or_default().into(),
                text: message.clone(),
                declaration: None,
                triggered: true,
            })
            .collect());
    }
    if rule.when.entity == "file" {
        let mut filters = globset::GlobSetBuilder::new();
        for path in &rule.applies_to.paths {
            filters.add(globset::Glob::new(path)?);
        }
        let filters = filters.build()?;
        return snapshot
            .changes
            .iter()
            .filter(|(path, delta)| {
                snapshot.files.contains_key(*path)
                    && snapshot.includes(path)
                    && (change == "any" || delta.kind == change)
                    && (filters.is_empty() || filters.is_match(path))
                    && (rule.language.is_empty()
                        || syntax::language(path).is_some_and(|language| {
                            rule.language.iter().any(|value| value == language)
                        }))
            })
            .map(|(path, _)| {
                Ok(Subject {
                    file: Some(path.clone()),
                    range: None,
                    identity: path.clone(),
                    name: path.clone(),
                    text: std::str::from_utf8(&snapshot.files[path].bytes)?.into(),
                    declaration: None,
                    triggered: true,
                })
            })
            .collect();
    }
    let files = entity_changes::collect(snapshot, &rule.applies_to.paths, &rule.language)?;
    let mut subjects = Vec::new();
    let mut previous_views = BTreeMap::new();
    let started = std::time::Instant::now();
    for file in files {
        if started.elapsed().as_secs() >= 30 {
            bail!("Marker retention exceeds analysis budget");
        }
        require_syntax_capabilities(&file.view.language, &rule.requires_capabilities)?;
        match rule.when.entity.as_str() {
            "test_method" => {
                let bytes = &snapshot.files[&file.path].bytes;
                for test in &file.tests {
                    let in_scope = rule.applies_to.provenance_scope.as_deref() != Some("ai_only")
                        || provenance
                            .is_some_and(|facts| facts.participated(&file.path, &test.entity));
                    let triggered = (change == "any" || test.kind == change) && in_scope;
                    let retained = if let (Some(binding), Some(previous), Some(path)) =
                        (&rule.binding, &test.previous, &test.previous_path)
                    {
                        if !previous_views.contains_key(path) {
                            previous_views.insert(
                                path.clone(),
                                syntax::parse(path, &snapshot.base_files[path].bytes)?
                                    .expect("previous supported syntax"),
                            );
                        }
                        markers::from_source(
                            &binding.marker,
                            previous,
                            &previous_views[path],
                            &snapshot.base_files[path].bytes,
                        )?
                        .is_some()
                    } else {
                        false
                    };
                    let declaration = rule
                        .binding
                        .as_ref()
                        .map(|binding| {
                            markers::declaration(
                                &binding.marker,
                                &test.entity,
                                &file.view,
                                &file.path,
                                snapshot,
                            )
                        })
                        .transpose()?
                        .flatten();
                    if !triggered && !retained && declaration.is_none() {
                        continue;
                    }
                    let entity = &test.entity;
                    subjects.push(Subject {
                        triggered,
                        file: Some(file.path.clone()),
                        range: Some(entity.range.clone()),
                        identity: entity.symbol.clone(),
                        name: entity.name.clone(),
                        text: std::str::from_utf8(&bytes[entity.byte_range.clone()])?.into(),
                        declaration,
                    });
                }
            }
            "comment" | "import" => {
                let entities = |view: &syntax::Structure| -> Vec<(String, Range)> {
                    if rule.when.entity == "comment" {
                        view.comments
                            .iter()
                            .map(|value| (value.text.clone(), value.range.clone()))
                            .collect()
                    } else {
                        view.imports
                            .iter()
                            .map(|value| (value.text.clone(), value.range.clone()))
                            .collect()
                    }
                };
                let mut counts = BTreeMap::new();
                if let Some(previous) = &file.previous {
                    for (text, _) in entities(previous) {
                        *counts.entry(text).or_insert(0usize) += 1;
                    }
                }
                for (text, range) in entities(&file.view) {
                    if change != "any"
                        && let Some(count) = counts.get_mut(&text)
                        && *count > 0
                    {
                        *count -= 1;
                        continue;
                    }
                    subjects.push(Subject {
                        triggered: true,
                        file: Some(file.path.clone()),
                        range: Some(range),
                        identity: text.clone(),
                        name: text.clone(),
                        text,
                        declaration: None,
                    });
                }
            }
            other => bail!("Unsupported DSL entity: {other}"),
        }
    }
    Ok(subjects)
}
