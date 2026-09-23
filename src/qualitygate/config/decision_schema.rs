//! Pinned machine-output schemas; no remote schema resolution or runtime fallback.

use anyhow::{Result, anyhow};
use serde_json::Value;
use std::sync::LazyLock;

const DECISION: &str =
    include_str!("../../../skills/qualitygate-cli/references/schemas/decision.schema.json");
const FEEDBACK: &str =
    include_str!("../../../skills/qualitygate-cli/references/schemas/feedback.schema.json");
const COMMAND_ERROR: &str =
    include_str!("../../../skills/qualitygate-cli/references/schemas/command-error.schema.json");

static DECISION_VALIDATOR: LazyLock<Result<jsonschema::Validator, String>> =
    LazyLock::new(|| compile(DECISION));
static FEEDBACK_VALIDATOR: LazyLock<Result<jsonschema::Validator, String>> =
    LazyLock::new(|| compile(FEEDBACK));
static COMMAND_ERROR_VALIDATOR: LazyLock<Result<jsonschema::Validator, String>> =
    LazyLock::new(|| compile(COMMAND_ERROR));

pub fn command_error_document() -> Result<Value> {
    COMMAND_ERROR_VALIDATOR
        .as_ref()
        .map_err(|error| anyhow!("Bundled command error schema is invalid: {error}"))?;
    Ok(serde_json::from_str(COMMAND_ERROR)?)
}

pub fn validate_command_error(value: &Value) -> Result<()> {
    let validator = COMMAND_ERROR_VALIDATOR
        .as_ref()
        .map_err(|error| anyhow!("Bundled command error schema is invalid: {error}"))?;
    validator
        .validate(value)
        .map_err(|error| anyhow!("Command error violates schema: {error}"))
}

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
