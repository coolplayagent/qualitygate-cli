use super::{CustomRule, constraints};
use anyhow::{Result, bail};
use std::collections::BTreeSet;

pub(super) fn validate(rule: &CustomRule) -> Result<()> {
    constraints::id(&rule.id)?;
    if rule.schema_version != 1 {
        bail!(
            "Unsupported custom rule schema_version: {}",
            rule.schema_version
        );
    }
    if rule.version == 0 {
        bail!("Custom rule version must be positive");
    }
    constraints::source(&rule.source)?;
    if rule.fix.trim().is_empty() {
        bail!("Custom rule needs actionable fix text");
    }
    let mut capabilities = BTreeSet::new();
    for capability in &rule.requires_capabilities {
        if ![
            "test_methods",
            "annotations",
            "comments",
            "imports",
            "files",
            "commits",
            "dependency_resolution",
            "external_provenance",
        ]
        .contains(&capability.as_str())
            || !capabilities.insert(capability)
        {
            bail!("Unknown or duplicate capability: {capability}");
        }
    }
    let required = match rule.when.entity.as_str() {
        "test_method" => "test_methods",
        "comment" => "comments",
        "import" => "imports",
        "file" => "files",
        "commit" => "commits",
        other => bail!("Unsupported custom rule entity: {other}"),
    };
    if !rule
        .requires_capabilities
        .iter()
        .any(|value| value == required)
    {
        bail!("Custom rule must declare capability {required}");
    }
    let change = rule.when.change.as_deref().unwrap_or("added");
    if !["added", "modified", "renamed", "any"].contains(&change)
        || rule.when.entity == "commit" && change != "added"
        || ["comment", "import"].contains(&rule.when.entity.as_str())
            && !["added", "any"].contains(&change)
    {
        bail!("Unsupported change {change} for {}", rule.when.entity);
    }
    if rule.when.entity == "commit"
        && (!rule.language.is_empty() || !rule.applies_to.paths.is_empty())
    {
        bail!("Commit rules cannot use file language or path filters");
    }
    for path in &rule.applies_to.paths {
        globset::Glob::new(path)?;
    }
    for language in &rule.language {
        if language.trim().is_empty() {
            bail!("Rule language cannot be empty");
        }
    }
    if !rule.then.require_marker
        && rule.then.require_dependency.is_none()
        && rule.then.name_pattern.is_none()
        && rule.then.forbid_pattern.is_none()
        && rule.then.max_count.is_none()
    {
        bail!("Custom rule must contain at least one assertion");
    }
    for pattern in [&rule.then.name_pattern, &rule.then.forbid_pattern]
        .into_iter()
        .flatten()
    {
        regex::Regex::new(pattern)?;
    }
    if rule.then.require_marker {
        if rule.when.entity != "test_method" {
            bail!("Marker assertions require test_method entities");
        }
        let marker = &rule
            .binding
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("require_marker requires an explicit binding"))?
            .marker;
        let capability = match marker.kind.as_str() {
            "annotation" => "annotations",
            "comment" => "comments",
            "git_trailer" => "commits",
            other => bail!("Unsupported marker binding: {other}"),
        };
        if !rule
            .requires_capabilities
            .iter()
            .any(|value| value == capability)
        {
            bail!("Marker binding requires declared capability {capability}");
        }
        if marker.name.trim().is_empty()
            || marker
                .fields
                .iter()
                .any(|field| constraints::id(field).is_err())
        {
            bail!("Marker name and field identifiers must be nonempty and valid");
        }
        if rule.applies_to.provenance_scope.is_none() {
            bail!("Marker assertion needs explicit provenance_scope");
        }
    } else if rule.binding.is_some() || rule.applies_to.provenance_scope.is_some() {
        bail!("Marker binding and provenance_scope require a marker assertion");
    }
    if let Some(scope) = &rule.applies_to.provenance_scope
        && !["all_added_tests", "ai_only"].contains(&scope.as_str())
    {
        bail!("Unsupported provenance_scope: {scope}");
    }
    if let Some(dependency) = &rule.then.require_dependency
        && (rule.when.entity != "test_method"
            || dependency.artifact.trim().is_empty()
            || !rule
                .requires_capabilities
                .iter()
                .any(|value| value == "dependency_resolution"))
    {
        bail!("Dependency assertions need test_method, artifact and dependency_resolution");
    }
    Ok(())
}
