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
pub const OPERATOR_SCHEMA_ID: &str = "urn:qualitygate:project-rule:2";
pub const FAMILY_SCHEMA_ID: &str = "urn:qualitygate:project-rule-family:1";
const OPERATOR_SCHEMA: &str =
    include_str!("../../../skills/qualitygate-cli/references/schemas/operator-rule.schema.json");

const V3_SCHEMA: &str =
    include_str!("../../../skills/qualitygate-cli/references/schemas/operator-rule-v3.schema.json");
pub const V3_SCHEMA_ID: &str = "urn:qualitygate:project-rule:3";
static V3_VALIDATOR: LazyLock<Result<jsonschema::Validator, String>> = LazyLock::new(|| {
    let schema: Value = serde_json::from_str(V3_SCHEMA).map_err(|e| e.to_string())?;
    jsonschema::draft202012::new(&schema).map_err(|e| e.to_string())
});
fn v3_validator() -> Result<&'static jsonschema::Validator> {
    V3_VALIDATOR
        .as_ref()
        .map_err(|e| anyhow::anyhow!("Bundled v3 schema could not be compiled: {e}"))
}
static OPERATOR_VALIDATOR: LazyLock<Result<jsonschema::Validator, String>> = LazyLock::new(|| {
    let schema: Value = serde_json::from_str(OPERATOR_SCHEMA).map_err(|e| e.to_string())?;
    jsonschema::draft202012::new(&schema).map_err(|e| e.to_string())
});

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

pub fn document_version(version: u32) -> Result<Value> {
    match version {
        1 => document(),
        2 => {
            operator_validator()?;
            Ok(serde_json::from_str(OPERATOR_SCHEMA)?)
        }
        3 => {
            v3_validator()?;
            Ok(serde_json::from_str(V3_SCHEMA)?)
        }
        _ => bail!("Unsupported project-rule schema version: {version}"),
    }
}
pub fn schema_identity(version: u32) -> Result<(&'static str, String)> {
    match version {
        1 => Ok((SCHEMA_ID, digest())),
        2 => Ok((
            OPERATOR_SCHEMA_ID,
            format!("sha256:{:x}", Sha256::digest(OPERATOR_SCHEMA.as_bytes())),
        )),
        3 => Ok((
            V3_SCHEMA_ID,
            format!("sha256:{:x}", Sha256::digest(V3_SCHEMA.as_bytes())),
        )),
        _ => bail!("Unsupported project-rule schema version: {version}"),
    }
}
fn operator_validator() -> Result<&'static jsonschema::Validator> {
    OPERATOR_VALIDATOR
        .as_ref()
        .map_err(|e| anyhow::anyhow!("Bundled operator schema could not be compiled: {e}"))
}

/// The authoring report for a mixed package binds both pinned validators.
pub fn family_document() -> Result<Value> {
    fn relocate(value: &mut Value, prefix: &str) {
        match value {
            Value::Object(map) => {
                if let Some(Value::String(reference)) = map.get_mut("$ref")
                    && let Some(local) = reference.strip_prefix("#/")
                {
                    *reference = format!("#/$defs/{prefix}/{local}");
                }
                for child in map.values_mut() {
                    relocate(child, prefix);
                }
            }
            Value::Array(values) => {
                for child in values {
                    relocate(child, prefix);
                }
            }
            _ => {}
        }
    }
    let mut definitions = serde_json::Map::new();
    for version in [1, 2, 3] {
        let name = format!("v{version}");
        let mut value = document_version(version)?;
        value
            .as_object_mut()
            .expect("pinned schema object")
            .remove("$id");
        relocate(&mut value, &name);
        definitions.insert(name, value);
    }
    let document = serde_json::json!({"$schema":"https://json-schema.org/draft/2020-12/schema","$id":FAMILY_SCHEMA_ID,
        "$defs":definitions,"oneOf":[{"$ref":"#/$defs/v1"},{"$ref":"#/$defs/v2"},{"$ref":"#/$defs/v3"}]});
    jsonschema::draft202012::new(&document)?;
    Ok(document)
}

pub fn family_digest() -> Result<String> {
    Ok(format!(
        "sha256:{:x}",
        Sha256::digest(serde_json::to_vec(&family_document()?)?)
    ))
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
    let (_, rule, issues) = inspect_versioned(bytes)?;
    Ok((rule, issues))
}

pub fn inspect_versioned(bytes: &[u8]) -> Result<(u32, Option<CustomRule>, Vec<RuleIssue>)> {
    if bytes.len() > super::MAX_CONFIG_BYTES {
        return Ok((
            1,
            None,
            vec![RuleIssue::new("input", "Project rule exceeds 1 MiB")],
        ));
    }
    let parsed = (|| -> Result<Value> {
        // Value rejects duplicate keys before typed maps or JSON conversion.
        let yaml = super::yaml_budget::value(bytes)?;
        yaml_shape(&yaml, 0, &mut 0)?;
        Ok(serde_json::to_value(yaml)?)
    })();
    let value = match parsed {
        Ok(value) => value,
        Err(error) => return Ok((1, None, vec![RuleIssue::new("yaml", format!("{error:#}"))])),
    };
    let version = match value.get("schema_version").and_then(Value::as_u64) {
        Some(2) => 2,
        Some(3) => 3,
        _ => 1,
    };
    let validator = match version {
        3 => v3_validator()?,
        2 => operator_validator()?,
        _ => validator()?,
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
        return Ok((version, None, issues));
    }
    let rule = (|| -> Result<CustomRule> {
        let rule: CustomRule = serde_json::from_value(value).context("Invalid typed rule")?;
        super::custom_validation::validate(&rule)?;
        Ok(rule)
    })();
    match rule {
        Ok(rule) => Ok((version, Some(rule), Vec::new())),
        Err(error) => Ok((
            version,
            None,
            vec![RuleIssue::new("semantic", format!("{error:#}"))],
        )),
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
