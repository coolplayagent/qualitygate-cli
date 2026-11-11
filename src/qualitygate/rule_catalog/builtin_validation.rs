use crate::config::Marker;
use crate::config::RuleSetting;
use crate::domain::rule_parameters::{self, LanguageContract, LanguageScope, PatternLanguages};
use anyhow::{Context, Result, bail};

pub(crate) fn validate_with_scope(
    builtin_id: &str,
    implementation: &str,
    builtin_languages: &[String],
    language_scope: Option<LanguageScope>,
    rule: &RuleSetting,
) -> Result<()> {
    let id = implementation;
    if id == "used-undeclared" {
        crate::rule_catalog::project_rules::used_undeclared(rule)?;
        return Ok(());
    }
    if id == "module-boundary" {
        crate::rule_catalog::project_rules::module_boundary(rule)?;
        return Ok(());
    }
    let specific = crate::rule_catalog::parameters::names(id)?;
    for (key, value) in &rule.parameters {
        if !specific.contains(&key.as_str()) {
            bail!("Unsupported {id} parameter: {key}");
        }
        match key.as_str() {
            "pattern" => {
                let pattern = value.as_str().context("pattern must be a string")?;
                if pattern.is_empty() || pattern.len() > 512 {
                    bail!("pattern must contain 1..512 bytes");
                }
                regex::Regex::new(pattern)?;
            }
            "annotation" => {
                let name = value.as_str().context("annotation must be a string")?;
                if name.is_empty()
                    || name.len() > 128
                    || !name.bytes().next().is_some_and(|byte| {
                        byte.is_ascii_alphabetic() || byte == b'_' || byte == b'$'
                    })
                    || !name
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$')
                {
                    bail!("annotation must be a simple Java annotation name of 1..128 bytes");
                }
            }
            "group" | "artifact" => {
                let text = value
                    .as_str()
                    .context("Maven coordinate must be a string")?;
                if text.is_empty()
                    || text.len() > 128
                    || !text.bytes().all(|byte| {
                        byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-')
                    })
                {
                    bail!("{key} must be a Maven coordinate component of 1..128 bytes");
                }
            }
            "patterns" => {
                for value in value
                    .as_object()
                    .context("patterns must map languages to regex strings")?
                    .values()
                {
                    regex::Regex::new(value.as_str().context("patterns values must be strings")?)?;
                }
            }
            "prohibited_patterns" | "forbidden_imports" => {
                rule_parameters::compile_patterns(
                    key,
                    value,
                    PatternLanguages::for_implementation(id),
                )?;
            }
            "paths" | "exempt_patterns" | "languages" => {
                for value in value
                    .as_array()
                    .with_context(|| format!("{key} must be an array"))?
                {
                    let text = value
                        .as_str()
                        .with_context(|| format!("{key} values must be strings"))?;
                    if key == "paths" {
                        if text.is_empty() {
                            bail!("paths values must be nonempty globs");
                        }
                        globset::Glob::new(text)?;
                    } else if key == "exempt_patterns" {
                        regex::Regex::new(text)?;
                    } else if !PatternLanguages::for_implementation(id).supports(text) {
                        bail!("Unsupported rule language: {text}");
                    }
                }
            }
            "max_added_lines" | "minimum_similar" => {
                let minimum = if key == "minimum_similar" { 2 } else { 1 };
                if value.as_u64().is_none_or(|number| number < minimum) {
                    bail!("{key} must be an integer >= {minimum}");
                }
            }
            "language" => {
                if value
                    .as_str()
                    .is_none_or(|value| !["chinese", "english", "bilingual"].contains(&value))
                {
                    bail!("comment-language requires chinese, english or bilingual");
                }
            }
            "provenance_scope" => {
                if value
                    .as_str()
                    .is_none_or(|value| !["all_added_tests", "ai_only"].contains(&value))
                {
                    bail!("provenance_scope requires all_added_tests or ai_only");
                }
            }
            "marker" => {
                let marker: Marker = serde_json::from_value(value.clone())?;
                if !["annotation", "comment", "git_trailer"].contains(&marker.kind.as_str())
                    || marker.name.trim().is_empty()
                {
                    bail!("Invalid marker binding");
                }
                for field in &marker.fields {
                    crate::config::constraints::id(field)?;
                }
            }
            _ => unreachable!("validated parameter key"),
        }
    }
    if id == "source-pattern" && !rule.parameters.contains_key("prohibited_patterns") {
        bail!("source-pattern requires prohibited_patterns");
    }
    LanguageContract::builtin(implementation, builtin_languages, language_scope)?
        .validate(builtin_id, &rule.parameters)?;
    if builtin_id == "no-test-sleep"
        && rule
            .parameters
            .get("paths")
            .and_then(serde_json::Value::as_array)
            .is_none_or(Vec::is_empty)
    {
        bail!("no-test-sleep requires at least one test path glob");
    }
    if id == "test-annotation-dependency"
        && ["annotation", "group", "artifact"]
            .iter()
            .any(|key| !rule.parameters.contains_key(*key))
    {
        bail!("test-annotation-dependency requires annotation, group and artifact");
    }
    if id == "file-pattern" && !rule.parameters.contains_key("pattern") {
        bail!("file-pattern requires pattern");
    }
    if id == "import-boundary" && rule.enabled && !rule.parameters.contains_key("forbidden_imports")
    {
        bail!("import-boundary requires forbidden_imports when enabled");
    }
    Ok(())
}

#[cfg(test)]
fn validate(
    builtin_id: &str,
    implementation: &str,
    builtin_languages: &[String],
    rule: &RuleSetting,
) -> Result<()> {
    validate_with_scope(builtin_id, implementation, builtin_languages, None, rule)
}

#[cfg(test)]
fn pattern_map(key: &str, value: &serde_json::Value, text_source: bool) -> Result<()> {
    rule_parameters::compile_patterns(
        key,
        value,
        if text_source {
            PatternLanguages::Text
        } else {
            PatternLanguages::Syntax
        },
    )
    .map(|_| ())
}

#[cfg(test)]
#[path = "builtin_validation_tests.rs"]
mod tests;
