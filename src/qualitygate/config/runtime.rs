//! YAML shape, explicit runtime requirements, then strict typed decoding.

use crate::domain::runtime::{CAPABILITIES, ConfigurationError, Requirements};
use anyhow::{Result, bail};
use serde::Deserialize;

pub fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    let _: serde_norway::Value =
        serde_norway::from_slice(bytes).map_err(|error| parser_error(error, "config.syntax"))?;
    Ok(serde_norway::from_slice(bytes).map_err(|error| parser_error(error, "config.type"))?)
}

pub(super) fn requirements(bytes: &[u8]) -> Result<()> {
    // Deserialize the envelope from the original bytes to retain line/column.
    // Unknown ordinary fields are handled by the subsequent strict Config parse.
    #[derive(Deserialize)]
    struct Envelope {
        #[serde(default)]
        requires: Requirements,
    }
    let envelope: Envelope = decode(bytes)?;
    validate(&envelope.requires)
}

pub fn validate(requirements: &Requirements) -> Result<()> {
    if let Some(minimum) = &requirements.min_cli_version {
        let version = semver::Version::parse(minimum).map_err(|error| ConfigurationError {
            code: "config.type".into(),
            message: format!("requires.min_cli_version requires a semantic version: {error}"),
            field: Some("requires.min_cli_version".into()),
            line: None,
            column: None,
        })?;
        if semver::Version::parse(env!("CARGO_PKG_VERSION"))? < version {
            return Err(ConfigurationError {
                code: "runtime.version_too_old".into(),
                message: format!(
                    "Policy requires CLI >= {minimum}; current CLI is {}",
                    env!("CARGO_PKG_VERSION")
                ),
                field: Some("requires.min_cli_version".into()),
                line: None,
                column: None,
            }
            .into());
        }
    }
    if requirements.capabilities.len() > 128 {
        bail!("requires.capabilities exceeds 128 identifiers");
    }
    let mut seen = std::collections::BTreeSet::new();
    for capability in &requirements.capabilities {
        if capability.is_empty() || capability.len() > 128 || !seen.insert(capability) {
            bail!("requires.capabilities needs distinct nonempty identifiers of at most 128 bytes");
        }
    }
    let missing: Vec<_> = requirements
        .capabilities
        .iter()
        .filter(|id| !CAPABILITIES.contains(&id.as_str()))
        .cloned()
        .collect();
    if !missing.is_empty() {
        return Err(ConfigurationError {
            code: "runtime.capability_missing".into(),
            message: format!(
                "Runtime lacks required capabilities: {}",
                missing.join(", ")
            ),
            field: Some("requires.capabilities".into()),
            line: None,
            column: None,
        }
        .into());
    }
    Ok(())
}

fn parser_error(error: serde_norway::Error, fallback: &str) -> ConfigurationError {
    let message = error.to_string();
    let field = message
        .split_once("unknown field `")
        .and_then(|(_, rest)| rest.split_once('`'))
        .map(|(field, _)| field.to_owned());
    let location = error.location();
    ConfigurationError {
        code: if field.is_some() {
            "config.unknown_field"
        } else {
            fallback
        }
        .into(),
        field,
        message,
        line: location.as_ref().map(|location| location.line()),
        column: location.as_ref().map(|location| location.column()),
    }
}

pub(super) fn validate_env(names: &[String]) -> Result<()> {
    let mut seen = std::collections::BTreeSet::new();
    if names.len() > 128 {
        bail!("required_env exceeds 128 names");
    }
    for name in names {
        if name.is_empty()
            || name.len() > 128
            || !seen.insert(name)
            || !name.bytes().enumerate().all(|(index, byte)| {
                byte == b'_' || byte.is_ascii_alphabetic() || (index > 0 && byte.is_ascii_digit())
            })
        {
            bail!("required_env needs distinct portable environment names: {name:?}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_and_environment_declarations_are_bounded_and_strict() {
        for capabilities in [
            vec!["".into()],
            vec!["x".repeat(129)],
            vec!["doctor.static.v1".into(); 2],
            vec!["x".into(); 129],
        ] {
            assert!(
                validate(&Requirements {
                    capabilities,
                    ..Default::default()
                })
                .is_err()
            );
        }
        validate_env(&["_TOOL_123".into(), "lowercase".into()]).unwrap();
        for names in [
            vec!["".into()],
            vec!["1NAME".into()],
            vec!["HAS=VALUE".into()],
            vec!["HAS SPACE".into()],
            vec!["名字".into()],
            vec!["X".repeat(129)],
            vec!["X".into(); 2],
            vec!["X".into(); 129],
        ] {
            assert!(validate_env(&names).is_err());
        }
        for requires in ["{min_cli_version: []}", "{capabilites: []}", "[]", "null"] {
            assert!(
                super::super::parse(
                    format!("schema_version: 1\nrequires: {requires}\n").as_bytes()
                )
                .is_err()
            );
        }
        let config = super::super::parse(b"schema_version: 1\nrequires: {}\n").unwrap();
        assert!(
            serde_json::to_value(config)
                .unwrap()
                .get("requires")
                .is_none()
        );
        assert!(
            super::super::parse(
                b"schema_version: 1\nchecks: [{id: manual, kind: manual, required_env: [X]}]\n"
            )
            .is_err()
        );
    }
}
