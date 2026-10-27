//! Finite DSL version 1 over snapshot, Git and language facts.

use super::{markers, rules::diagnostic, syntax};
use crate::{
    config::{CustomRule, RuleSetting},
    domain::*,
    snapshot::Snapshot,
};
use anyhow::{Result, bail};
use regex::Regex;
use std::collections::BTreeMap;

mod expression_v3;
mod file_inventory;
mod selection;

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
    evaluate_with_context(
        rule,
        setting,
        snapshot,
        &super::facts::RuleFacts {
            projects,
            provenance,
            git_trailers: None,
        },
    )
}

pub fn evaluate_with_context(
    rule: &CustomRule,
    setting: &RuleSetting,
    snapshot: &Snapshot,
    facts: &super::facts::RuleFacts<'_>,
) -> CheckResult {
    let provenance = facts.provenance;
    let mut result = CheckResult::pending(&rule.id, setting.required, setting.severity);
    result.rule_version = rule.version;
    let evaluation = (|| -> Result<()> {
        if let Some(facts) = provenance {
            facts.ensure_binding(snapshot, &rule.id)?;
        }
        if setting.provenance.is_some() && provenance.is_none() {
            bail!("Configured provenance evidence is unavailable");
        }
        if let Some(git) = facts.git_trailers {
            git.ensure_binding(snapshot)?;
            result
                .metadata
                .insert("git_trailers".into(), git.evidence().clone());
        }
        run(rule, snapshot, facts, &mut result)
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
    facts: &super::facts::RuleFacts<'_>,
    result: &mut CheckResult,
) -> Result<()> {
    let mut budget = (rule.schema_version == 3).then(selection::Budget::new);
    requirements(rule, snapshot, facts)?;
    let projects = facts.projects;
    let subjects = selection::collect(rule, snapshot, facts, false, budget.as_mut())?;
    let triggered = subjects.iter().filter(|subject| subject.triggered).count();
    if rule.when.entity == "file" && rule.when.change.as_deref() == Some("all") {
        file_inventory::assert(rule, snapshot, &subjects, result)?;
    }
    result.matched_entities = subjects.len();
    result.metadata.insert(
        "retained_marker_entities".into(),
        serde_json::json!(subjects.len() - triggered),
    );
    result.metadata.insert(
        "increment_mode".into(),
        serde_json::json!(if rule.when.change.as_deref() == Some("all") {
            "full_files"
        } else {
            "entity_changes"
        }),
    );
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
    let require = rule
        .then
        .require_pattern
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
        if subject.triggered
            && require
                .as_ref()
                .is_some_and(|pattern| !pattern.is_match(&subject.text))
        {
            violations.push((
                "require_pattern",
                "Entity lacks text required by this rule".into(),
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
        dependencies(
            rule,
            snapshot,
            projects,
            &subjects,
            result,
            facts.git_trailers,
        )?;
    }
    if !(rule.when.entity == "file" && rule.when.change.as_deref() == Some("all"))
        && let Some(minimum) = rule.then.min_count
        && triggered < minimum
    {
        result.diagnostics.push(diagnostic(&rule.id, None, None,
            format!("{triggered} matching entities is below min_count {minimum}"),
            serde_json::json!({"assertion":"min_count", "count":triggered,"limit":minimum,"entity":rule.when.entity}),
            &rule.fix, "min_count"));
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
    if rule.schema_version == 3 {
        expression_v3::evaluate(
            rule,
            snapshot,
            facts,
            &subjects,
            result,
            budget.as_mut().expect("v3 acquisition budget"),
        )?;
    } else if let Some(expression) = &rule.expression {
        let entities: Vec<_> = subjects
            .iter()
            .filter(|subject| subject.triggered)
            .map(|subject| super::operators::Entity {
                name: &subject.name,
                text: &subject.text,
                path: subject.file.as_deref(),
                range: subject.range.as_ref(),
            })
            .collect();
        let outcome = super::operators::evaluate(
            &expression.legacy().map_err(anyhow::Error::msg)?,
            super::operators::Facts {
                entities: &entities,
                complete: true,
            },
        )?;
        if !outcome.passed {
            result.diagnostics.push(diagnostic(
                &rule.id,
                None,
                None,
                "Typed expression did not hold for the complete selected entity collection".into(),
                serde_json::json!({"assertion":"expression","outcome":outcome}),
                &rule.fix,
                "expression",
            ));
        }
        result
            .metadata
            .insert("expression".into(), serde_json::to_value(outcome)?);
    }
    Ok(())
}

fn requirements(
    rule: &CustomRule,
    snapshot: &Snapshot,
    facts: &super::facts::RuleFacts<'_>,
) -> Result<()> {
    let (projects, provenance) = (facts.projects, facts.provenance);
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
        && facts.git_trailers.is_none()
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
    if rule.schema_version >= 2
        && rule
            .requires_capabilities
            .iter()
            .any(|c| c == "dependency_resolution")
        && projects.iter().any(|project| {
            project.schema_version != 1
                || !["maven", "python"].contains(&project.ecosystem.as_str())
                || project.snapshot_digest != snapshot.identity.content_digest
        })
    {
        bail!("Required project facts are unsupported or belong to another snapshot");
    }
    Ok(())
}

fn dependencies(
    rule: &CustomRule,
    snapshot: &Snapshot,
    projects: &[ProjectFacts],
    subjects: &[Subject],
    result: &mut CheckResult,
    git: Option<&super::git_trailers::GitFacts>,
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
                    if markers::bound_declaration(
                        &binding.marker,
                        test,
                        &view,
                        path,
                        snapshot,
                        false,
                        git,
                    )?
                    .is_some()
                    {
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
