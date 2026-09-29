//! Bundled, offline schemas for runtime identity and preflight output.

use anyhow::{Result, ensure};
use serde_json::Value;

const CAPABILITIES: &str =
    include_str!("../../../skills/qualitygate-cli/references/schemas/capabilities.schema.json");
const DOCTOR: &str =
    include_str!("../../../skills/qualitygate-cli/references/schemas/doctor.schema.json");

pub fn capabilities_document() -> Result<Value> {
    document(CAPABILITIES)
}
pub fn doctor_document() -> Result<Value> {
    document(DOCTOR)
}

fn document(source: &str) -> Result<Value> {
    let schema = serde_json::from_str(source)?;
    jsonschema::draft202012::new(&schema)?;
    Ok(schema)
}

pub fn validate_capabilities(value: &Value) -> Result<()> {
    validate(CAPABILITIES, value)
}
pub fn validate_doctor(value: &Value) -> Result<()> {
    validate(DOCTOR, value)
}

fn validate(source: &str, value: &Value) -> Result<()> {
    let schema = document(source)?;
    let validator = jsonschema::draft202012::new(&schema)?;
    let errors: Vec<_> = validator
        .iter_errors(value)
        .take(8)
        .map(|error| error.to_string())
        .collect();
    ensure!(
        errors.is_empty(),
        "Preflight output violates schema: {}",
        errors.join("; ")
    );
    Ok(())
}
