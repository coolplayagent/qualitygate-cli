use crate::config::CustomRule;
use crate::config::constraints;
use anyhow::{Result, bail};
use std::collections::BTreeSet;

pub(crate) fn validate(rule: &CustomRule) -> Result<()> {
    constraints::id(&rule.id)?;
    if ![1, 2, 3].contains(&rule.schema_version) {
        bail!(
            "Unsupported custom rule schema_version: {}",
            rule.schema_version
        );
    }
    match (rule.schema_version, &rule.expression) {
        (1, Some(_)) => bail!("Project-rule v1 cannot contain expression"),
        (2 | 3, None) => bail!("Project-rule v2/v3 requires a typed expression"),
        (_, Some(expression)) => {
            if rule.schema_version == 3 {
                expression
                    .validate(&rule.sources.keys().cloned().collect())
                    .map_err(anyhow::Error::msg)?;
            } else {
                expression
                    .legacy()
                    .map_err(anyhow::Error::msg)?
                    .validate()
                    .map_err(anyhow::Error::msg)?;
            }
        }
        _ => {}
    }
    if rule.schema_version != 3 && !rule.sources.is_empty() {
        bail!("Named sources require project-rule v3");
    }
    for source in rule.sources.values() {
        validate(&rule.for_source(source))?;
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
    if !["added", "modified", "renamed", "any", "all"].contains(&change)
        || change == "all" && rule.when.entity != "file"
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
    if rule.expression.is_none()
        && !rule.then.require_marker
        && rule.then.require_dependency.is_none()
        && rule.then.name_pattern.is_none()
        && rule.then.forbid_pattern.is_none()
        && rule.then.require_pattern.is_none()
        && rule.then.max_count.is_none()
        && rule.then.min_count.is_none()
        && rule.then.max_lines.is_none()
        && rule.then.max_total_words.is_none()
        && rule.then.required_paths.is_empty()
    {
        bail!("Custom rule must contain at least one assertion");
    }
    if (rule.then.max_lines.is_some()
        || rule.then.max_total_words.is_some()
        || !rule.then.required_paths.is_empty())
        && (rule.when.entity != "file" || change != "all")
    {
        bail!("File inventory assertions require file entities with change: all");
    }
    if let (Some(minimum), Some(maximum)) = (rule.then.min_count, rule.then.max_count)
        && minimum > maximum
    {
        bail!("min_count exceeds max_count");
    }
    if rule.then.required_paths.len() > 256 {
        bail!("required_paths exceeds 256 files");
    }
    let mut paths = BTreeSet::new();
    for path in &rule.then.required_paths {
        let normalized = crate::paths::relative(std::path::Path::new(path))?;
        if path.is_empty()
            || normalized.is_empty()
            || normalized != *path
            || path.contains(['*', '?', '[', ']', '{', '}'])
            || !paths.insert(path)
        {
            bail!("required_paths must contain unique normalized literal file paths");
        }
    }
    for pattern in [
        &rule.then.name_pattern,
        &rule.then.forbid_pattern,
        &rule.then.require_pattern,
    ]
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

#[cfg(test)]
mod scenario_tests {
    use super::*;
    use serde_json::json;
    fn rule() -> CustomRule {
        serde_json::from_value(json!({"id":"meaningful-tests","version":1,"source":{"document":"AGENTS.md","section":"Tests","content_hash":format!("sha256:{}","0".repeat(64))},"language":["python"],"requires_capabilities":["test_methods","comments"],"when":{"entity":"test_method"},"then":{"min_count":1},"fix":"Add a scenario test"})).unwrap()
    }
    #[test]
    fn schema_generation_and_marker_scope_cannot_silently_change_assertion_semantics() {
        let expression = json!({"op":"compare","left":{"op":"count","input":{"op":"entities"}},"relation":"ge","right":{"op":"number","value":1}});
        for (case, message) in [
            (0, "Unsupported custom rule schema"),
            (1, "v1 cannot"),
            (2, "requires a typed expression"),
            (3, "Named sources require"),
            (4, "Commit rules cannot"),
            (5, "test_method entities"),
            (6, "explicit provenance_scope"),
            (7, "require a marker assertion"),
        ] {
            let mut candidate = rule();
            match case {
                0=>candidate.schema_version=99,
                1=>candidate.expression=Some(serde_json::from_value(expression.clone()).unwrap()),
                2=>candidate.schema_version=2,
                3=>candidate.sources=serde_json::from_value(json!({"tests":{"when":{"entity":"file","change":"all"},"requires_capabilities":["files"]}})).unwrap(),
                4=>{candidate.when.entity="commit".into();candidate.requires_capabilities.push("commits".into());},
                5=>{candidate.when.entity="comment".into();candidate.then.require_marker=true;},
                6=>{candidate.then.require_marker=true;candidate.binding=Some(serde_json::from_value(json!({"marker":{"type":"comment","name":"Generated","fields":[]}})).unwrap());},
                _=>candidate.applies_to.provenance_scope=Some("all_added_tests".into()),
            }
            assert!(
                validate(&candidate)
                    .unwrap_err()
                    .to_string()
                    .contains(message),
                "{case}"
            );
        }
        for version in [2, 3] {
            let mut candidate = rule();
            candidate.schema_version = version;
            candidate.expression = Some(serde_json::from_value(expression.clone()).unwrap());
            if version == 3 {
                candidate.sources=serde_json::from_value(json!({"tests":{"when":{"entity":"file","change":"all"},"requires_capabilities":["files"]}})).unwrap();
            }
            validate(&candidate).unwrap();
            let bytes = serde_json::to_vec(&candidate).unwrap();
            let (actual, parsed, issues) =
                crate::config::rule_schema::inspect_versioned(&bytes).unwrap();
            assert_eq!(actual, version);
            assert!(issues.is_empty(), "{issues:?}");
            assert_eq!(parsed.unwrap().id, candidate.id);
        }
    }
}
