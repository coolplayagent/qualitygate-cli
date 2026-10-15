//! Bounded clause validation and review bindings in the selected policy.
use crate::domain::constraint::{CHECK_ID, Constraint, Realization};
use anyhow::{Result, ensure};
use std::collections::{BTreeMap, BTreeSet};

pub fn binding(
    constraint: &Constraint,
    config: &super::Config,
    catalog: &super::catalog::Catalog,
) -> Result<String> {
    let mut subject = constraint.clone();
    subject.adoption = None;
    let mut definitions = BTreeMap::new();
    if let Realization::Checks { checks, .. } = &constraint.realization {
        for id in checks {
            let definition = if let Some(setting) = config.rules.get(id) {
                serde_json::json!({"setting":setting,"definition":catalog.entries.get(id)})
            } else {
                serde_json::json!({"command":config.checks.iter().find(|check| &check.id == id)})
            };
            definitions.insert(id, definition);
        }
    }
    Ok(super::policy_store::digest(&serde_json::to_vec(
        &serde_json::json!({"constraint":subject,"checks":definitions}),
    )?))
}

pub fn bindings(
    config: &super::Config,
    catalog: &super::catalog::Catalog,
) -> Result<BTreeMap<String, String>> {
    config
        .constraints
        .iter()
        .map(|c| Ok((c.id.clone(), binding(c, config, catalog)?)))
        .collect()
}

pub(super) fn validate(config: &super::Config) -> Result<()> {
    ensure!(
        config.constraints.len() <= 256,
        "Constraint inventory exceeds 256 clauses"
    );
    ensure!(
        !config.rules.contains_key(CHECK_ID) && !config.checks.iter().any(|c| c.id == CHECK_ID),
        "Check ID {CHECK_ID} is reserved for constraint coverage"
    );
    let mut ids = BTreeSet::new();
    let mut reviews = BTreeSet::new();
    for c in &config.constraints {
        super::constraints::id(&c.id)?;
        ensure!(ids.insert(&c.id), "Duplicate constraint ID: {}", c.id);
        ensure!(
            c.schema_version == 1 && c.version > 0,
            "Constraint requires schema_version 1 and positive version"
        );
        text(&c.statement, 4096)?;
        super::constraints::source(&c.source)?;
        ensure!(
            crate::paths::relative(c.source.document.as_ref())? == c.source.document
                && !c.source.document.is_empty()
                && c.source.document != ".",
            "Constraint source must be a normalized repository file"
        );
        text(&c.source.section, 1024)?;
        if let Some(adoption) = &c.adoption {
            adoption.validate().map_err(anyhow::Error::msg)?;
            ensure!(
                reviews.insert(&adoption.id),
                "Constraint adoption IDs must be unique"
            );
        }
        match &c.realization {
            Realization::Checks {
                checks,
                positive_examples,
                negative_examples,
                ..
            } => {
                ensure!(
                    !checks.is_empty() && checks.len() <= 64,
                    "Constraint mapping needs 1..64 checks"
                );
                let mut unique = BTreeSet::new();
                for id in checks {
                    super::constraints::id(id)?;
                    ensure!(
                        id != CHECK_ID && unique.insert(id),
                        "Constraint checks must be distinct and cannot refer to coverage itself"
                    );
                    ensure!(
                        config.rules.contains_key(id)
                            || config.checks.iter().any(|check| &check.id == id),
                        "Constraint {} maps an unknown policy check: {id}",
                        c.id
                    );
                }
                examples(positive_examples)?;
                examples(negative_examples)?;
            }
            Realization::Unsupported {
                reason,
                evidence_owner,
            } => {
                text(reason, 4096)?;
                text(evidence_owner, 1024)?;
            }
        }
    }
    Ok(())
}

fn text(value: &str, max: usize) -> Result<()> {
    ensure!(
        !value.trim().is_empty() && value.len() <= max && !value.chars().any(char::is_control),
        "Constraint text must be nonempty, bounded and contain no control characters"
    );
    Ok(())
}

fn examples(values: &[String]) -> Result<()> {
    ensure!(
        !values.is_empty() && values.len() <= 16,
        "Constraint needs 1..16 references for each example polarity"
    );
    let mut unique = BTreeSet::new();
    for value in values {
        text(value, 1024)?;
        ensure!(
            unique.insert(value),
            "Duplicate constraint example reference"
        );
    }
    Ok(())
}

pub fn schema() -> Result<serde_json::Value> {
    let value = serde_json::from_str(include_str!(
        "../../../skills/qualitygate-cli/references/schemas/constraint.schema.json"
    ))?;
    jsonschema::draft202012::new(&value)?;
    Ok(value)
}
