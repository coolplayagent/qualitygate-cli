//! The Skill's shipped JSON Schema is the executable project-rule contract.

use super::CustomRule;
use crate::domain::RuleIssue;
use anyhow::{Context, Result, bail};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::LazyLock;

pub const SCHEMA_ID: &str = "urn:qualitygate:project-rule:1";
pub const SCHEMA_PATH: &str = "references/schemas/project-rule.schema.json";
pub const SCHEMA: &str =
    include_str!("../../../skills/qualitygate-cli/references/schemas/project-rule.schema.json");

static VALIDATOR: LazyLock<Result<jsonschema::Validator, String>> = LazyLock::new(|| {
    let schema: Value = serde_json::from_str(SCHEMA).map_err(|error| error.to_string())?;
    // Only the pinned bundled schema is compiled. HTTP/file resolution features
    // are disabled; a candidate cannot supply a replacement schema or retriever.
    jsonschema::draft202012::options()
        .build(&schema)
        .map_err(|error| error.to_string())
});

pub fn digest() -> String {
    format!("sha256:{:x}", Sha256::digest(SCHEMA.as_bytes()))
}

pub fn document() -> Result<Value> {
    validator()?;
    Ok(serde_json::from_str(SCHEMA)?)
}

fn validator() -> Result<&'static jsonschema::Validator> {
    VALIDATOR.as_ref().map_err(|error| {
        anyhow::anyhow!("Bundled project rule schema could not be compiled: {error}")
    })
}

fn yaml_shape(value: &serde_norway::Value, depth: usize, nodes: &mut usize) -> Result<()> {
    *nodes += 1;
    if depth > 64 || *nodes > 50_000 {
        bail!("Project rule exceeds YAML depth/node budget");
    }
    match value {
        serde_norway::Value::Mapping(mapping) => {
            for (key, value) in mapping {
                if !matches!(key, serde_norway::Value::String(_)) {
                    bail!("Project rule YAML mappings require string keys");
                }
                yaml_shape(value, depth + 1, nodes)?;
            }
        }
        serde_norway::Value::Sequence(sequence) => {
            for value in sequence {
                yaml_shape(value, depth + 1, nodes)?;
            }
        }
        serde_norway::Value::Tagged(_) => bail!("Project rule YAML tags are unsupported"),
        serde_norway::Value::Number(number)
            if number.as_f64().is_some_and(|number| !number.is_finite()) =>
        {
            bail!("Project rule YAML numbers must be finite");
        }
        _ => {}
    }
    Ok(())
}

pub fn inspect(bytes: &[u8]) -> Result<(Option<CustomRule>, Vec<RuleIssue>)> {
    let validator = validator()?;
    if bytes.len() > super::MAX_CONFIG_BYTES {
        return Ok((
            None,
            vec![RuleIssue::new("input", "Project rule exceeds 1 MiB")],
        ));
    }
    let parsed = (|| -> Result<Value> {
        // Value rejects duplicate keys before typed maps or JSON conversion.
        let yaml: serde_norway::Value = serde_norway::from_slice(bytes)?;
        yaml_shape(&yaml, 0, &mut 0)?;
        Ok(serde_json::to_value(yaml)?)
    })();
    let value = match parsed {
        Ok(value) => value,
        Err(error) => return Ok((None, vec![RuleIssue::new("yaml", format!("{error:#}"))])),
    };
    let issues: Vec<_> = validator
        .iter_errors(&value)
        .take(16)
        .map(|error| RuleIssue {
            instance_path: error.instance_path().as_str().chars().take(512).collect(),
            schema_path: Some(error.schema_path().as_str().into()),
            ..RuleIssue::new("schema", error.to_string())
        })
        .collect();
    if !issues.is_empty() {
        return Ok((None, issues));
    }
    let rule = (|| -> Result<CustomRule> {
        let rule: CustomRule = serde_json::from_value(value).context("Invalid typed rule")?;
        super::custom_validation::validate(&rule)?;
        Ok(rule)
    })();
    match rule {
        Ok(rule) => Ok((Some(rule), Vec::new())),
        Err(error) => Ok((None, vec![RuleIssue::new("semantic", format!("{error:#}"))])),
    }
}

/// Used by both rule authoring and immutable policy snapshot loading.
pub fn parse(bytes: &[u8]) -> Result<CustomRule> {
    let (rule, issues) = inspect(bytes)?;
    match rule {
        Some(rule) => Ok(rule),
        None => bail!(
            "Project rule schema/semantic validation failed: {}",
            issues
                .iter()
                .map(|issue| format!(
                    "{} at {}: {}",
                    issue.stage, issue.instance_path, issue.message
                ))
                .collect::<Vec<_>>()
                .join("; ")
        ),
    }
}
