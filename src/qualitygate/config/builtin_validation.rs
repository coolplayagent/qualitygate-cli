use super::{Marker, RuleSetting};
use anyhow::{Context, Result, bail};

pub(super) fn validate(id: &str, rule: &RuleSetting) -> Result<()> {
    if id == "module-boundary" {
        super::project_rules::module_boundary(rule)?;
        return Ok(());
    }
    let specific: &[&str] = match id {
        "line-ending" => &[],
        "commit-message" => &["pattern"],
        "diff-size" => &["max_added_lines"],
        "test-naming" => &["pattern", "patterns", "paths", "languages"],
        "parameterized-tests" => &["minimum_similar", "paths", "languages"],
        "comment-language" => &["language", "exempt_patterns", "paths", "languages"],
        "ai-code-traceability" => &["marker", "provenance_scope", "paths", "languages"],
        _ => bail!("Unknown builtin implementation: {id}"),
    };
    for (key, value) in &rule.parameters {
        if !specific.contains(&key.as_str()) {
            bail!("Unsupported {id} parameter: {key}");
        }
        match key.as_str() {
            "pattern" => {
                regex::Regex::new(value.as_str().context("pattern must be a string")?)?;
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
            "paths" | "exempt_patterns" | "languages" => {
                for value in value
                    .as_array()
                    .with_context(|| format!("{key} must be an array"))?
                {
                    let text = value
                        .as_str()
                        .with_context(|| format!("{key} values must be strings"))?;
                    if key == "paths" {
                        globset::Glob::new(text)?;
                    } else if key == "exempt_patterns" {
                        regex::Regex::new(text)?;
                    } else if !["java", "python", "typescript", "go", "rust", "shell"]
                        .contains(&text)
                    {
                        bail!("Unsupported syntax language: {text}");
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
    Ok(())
}
