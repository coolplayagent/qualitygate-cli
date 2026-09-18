use super::{Marker, RuleSetting};
use anyhow::{Context, Result, bail};
use std::collections::BTreeSet;

pub(super) fn validate(
    builtin_id: &str,
    implementation: &str,
    builtin_languages: &[String],
    rule: &RuleSetting,
) -> Result<()> {
    let id = implementation;
    if id == "used-undeclared" {
        super::project_rules::used_undeclared(rule)?;
        return Ok(());
    }
    if id == "module-boundary" {
        super::project_rules::module_boundary(rule)?;
        return Ok(());
    }
    let specific = super::parameters::names(id)?;
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
                pattern_map(key, value)?;
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
                    } else if !["java", "python", "typescript", "go", "rust", "shell"]
                        .contains(&text)
                        && !(id == "file-pattern" && ["c", "cpp"].contains(&text))
                    {
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
                    super::constraints::id(field)?;
                }
            }
            _ => unreachable!("validated parameter key"),
        }
    }
    if id == "source-pattern" && !rule.parameters.contains_key("prohibited_patterns") {
        bail!("source-pattern requires prohibited_patterns");
    }
    if ["test-naming-strict", "test-annotation-dependency"].contains(&id)
        && rule.parameters.get("languages") != Some(&serde_json::json!(["java"]))
    {
        bail!("{id} requires languages: [java]");
    }
    if builtin_id == "python-test-naming"
        && rule.parameters.get("languages") != Some(&serde_json::json!(["python"]))
    {
        bail!("python-test-naming requires languages: [python]");
    }
    if id == "file-pattern"
        && rule.parameters.get("languages") != Some(&serde_json::json!(builtin_languages))
    {
        bail!("{builtin_id} requires languages: {builtin_languages:?}");
    }
    if ["shell-shebang", "shell-commented-code"].contains(&id)
        && rule.parameters.get("languages") != Some(&serde_json::json!(["shell"]))
    {
        bail!("{builtin_id} requires languages: [shell]");
    }
    let fixed_language = if builtin_id.starts_with("shell-") {
        Some("shell")
    } else if builtin_id.starts_with("rust-") {
        Some("rust")
    } else if builtin_id.starts_with("ts-") {
        Some("typescript")
    } else {
        None
    };
    if let Some(language) = fixed_language.filter(|_| id == "source-pattern") {
        if rule.parameters.get("languages") != Some(&serde_json::json!([language])) {
            bail!("{builtin_id} requires languages: [{language}]");
        }
        let patterns = rule
            .parameters
            .get("prohibited_patterns")
            .and_then(serde_json::Value::as_object);
        if patterns.is_none_or(|patterns| patterns.len() != 1 || !patterns.contains_key(language)) {
            bail!("{builtin_id} requires prohibited_patterns.{language} only");
        }
    }
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

fn pattern_map(key: &str, value: &serde_json::Value) -> Result<()> {
    let values = value
        .as_object()
        .with_context(|| format!("{key} must map languages to regex arrays"))?;
    if values.is_empty() || values.len() > 32 {
        bail!("{key} requires 1..32 language entries");
    }
    for (language, entries) in values {
        if language != "all"
            && !["java", "python", "typescript", "go", "rust", "shell"].contains(&language.as_str())
        {
            bail!("{key} has unsupported language: {language}");
        }
        let entries = entries
            .as_array()
            .with_context(|| format!("{key}.{language} must be a regex array"))?;
        if entries.is_empty() || entries.len() > 32 {
            bail!("{key}.{language} requires 1..32 regex patterns");
        }
        let mut unique = BTreeSet::new();
        for pattern in entries {
            let pattern = pattern
                .as_str()
                .with_context(|| format!("{key}.{language} values must be strings"))?;
            if pattern.is_empty() || pattern.len() > 512 || !unique.insert(pattern) {
                bail!("{key}.{language} requires distinct regex patterns of 1..512 bytes");
            }
            regex::Regex::new(pattern)?;
        }
    }
    Ok(())
}
