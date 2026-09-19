//! Pinned machine-output schemas; no remote schema resolution or runtime fallback.

use anyhow::{Result, anyhow};
use serde_json::Value;
use std::sync::LazyLock;

const DECISION: &str =
    include_str!("../../../skills/qualitygate-cli/references/schemas/decision.schema.json");
const FEEDBACK: &str =
    include_str!("../../../skills/qualitygate-cli/references/schemas/feedback.schema.json");

static DECISION_VALIDATOR: LazyLock<Result<jsonschema::Validator, String>> =
    LazyLock::new(|| compile(DECISION));
static FEEDBACK_VALIDATOR: LazyLock<Result<jsonschema::Validator, String>> =
    LazyLock::new(|| compile(FEEDBACK));

fn compile(source: &str) -> Result<jsonschema::Validator, String> {
    let schema: Value = serde_json::from_str(source).map_err(|error| error.to_string())?;
    jsonschema::draft202012::options()
        .build(&schema)
        .map_err(|error| error.to_string())
}

pub fn document() -> Result<Value> {
    DECISION_VALIDATOR
        .as_ref()
        .map_err(|error| anyhow!("Bundled decision schema is invalid: {error}"))?;
    Ok(serde_json::from_str(DECISION)?)
}

pub fn feedback_document() -> Result<Value> {
    FEEDBACK_VALIDATOR
        .as_ref()
        .map_err(|error| anyhow!("Bundled feedback schema is invalid: {error}"))?;
    Ok(serde_json::from_str(FEEDBACK)?)
}

pub fn validate_decision(value: &Value) -> Result<()> {
    let validator = DECISION_VALIDATOR
        .as_ref()
        .map_err(|error| anyhow!("Bundled decision schema is invalid: {error}"))?;
    let errors: Vec<_> = validator
        .iter_errors(value)
        .take(8)
        .map(|error| error.to_string())
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(anyhow!(
            "Decision envelope violates schema: {}",
            errors.join("; ")
        ))
    }
}

pub fn validate_feedback(value: &Value) -> Result<()> {
    let validator = FEEDBACK_VALIDATOR
        .as_ref()
        .map_err(|error| anyhow!("Bundled feedback schema is invalid: {error}"))?;
    let errors: Vec<_> = validator
        .iter_errors(value)
        .take(8)
        .map(|error| error.to_string())
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(anyhow!("Feedback violates schema: {}", errors.join("; ")))
    }
}
