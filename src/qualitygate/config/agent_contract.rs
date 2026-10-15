//! Strict behavior obligation validation and adoption bindings.
use crate::domain::agent_contract::{Enforcement, Obligation};
use anyhow::{Result, ensure};
use std::collections::{BTreeMap, BTreeSet};

pub fn binding(
    obligation: &Obligation,
    config: &super::Config,
    catalog: &super::catalog::Catalog,
) -> Result<String> {
    let mut subject = obligation.clone();
    subject.adoption = None;
    let mut definitions = BTreeMap::new();
    if let Enforcement::Checks { checks } = &obligation.enforcement {
        for id in checks {
            definitions.insert(id, serde_json::json!({"setting":config.rules.get(id),
                "definition":catalog.entries.get(id),"command":config.checks.iter().find(|c| &c.id == id)}));
        }
    }
    Ok(super::policy_store::digest(&serde_json::to_vec(
        &serde_json::json!({"obligation":subject,"checks":definitions}),
    )?))
}

pub fn bindings(
    config: &super::Config,
    catalog: &super::catalog::Catalog,
) -> Result<BTreeMap<String, String>> {
    config
        .agent_obligations
        .iter()
        .map(|o| Ok((o.id.clone(), binding(o, config, catalog)?)))
        .collect()
}

pub(super) fn validate(config: &super::Config) -> Result<()> {
    ensure!(
        config.agent_obligations.len() <= 128,
        "Agent contract exceeds 128 obligations"
    );
    let mut ids = BTreeSet::new();
    let mut reviews = BTreeSet::new();
    for o in &config.agent_obligations {
        super::constraints::id(&o.id)?;
        ensure!(ids.insert(&o.id), "Duplicate agent obligation ID: {}", o.id);
        ensure!(
            o.schema_version == 1 && o.version > 0,
            "Agent obligation needs schema_version 1 and positive version"
        );
        super::constraints::source(&o.source)?;
        ensure!(
            crate::paths::relative(o.source.document.as_ref())? == o.source.document
                && !o.source.document.is_empty()
                && o.source.document != ".",
            "Agent source must be a normalized repository file"
        );
        text(&o.source.section, 1024)?;
        identifiers(&o.scope.roles, 32)?;
        identifiers(&o.scope.tasks, 32)?;
        ensure!(
            !o.triggers.is_empty()
                && o.triggers.len() <= 7
                && o.triggers.iter().collect::<BTreeSet<_>>().len() == o.triggers.len(),
            "Agent triggers must be nonempty and distinct"
        );
        texts(&o.must)?;
        texts(&o.must_not)?;
        ensure!(
            !o.must.is_empty() || !o.must_not.is_empty(),
            "Agent obligation requires an instruction"
        );
        if let Some(adoption) = &o.adoption {
            adoption.validate().map_err(anyhow::Error::msg)?;
            ensure!(
                reviews.insert(&adoption.id),
                "Agent adoption IDs must be distinct"
            );
        }
        if let Enforcement::Checks { checks } = &o.enforcement {
            identifiers(checks, 64)?;
            ensure!(
                !checks.is_empty(),
                "Agent evidence needs at least one check"
            );
            for id in checks {
                ensure!(
                    config.rules.contains_key(id) || config.checks.iter().any(|c| &c.id == id),
                    "Agent obligation maps an unknown check: {id}"
                );
            }
        }
    }
    Ok(())
}

pub fn role(value: &str) -> Result<()> {
    super::constraints::id(value)
}

fn identifiers(values: &[String], max: usize) -> Result<()> {
    ensure!(values.len() <= max, "Agent identifier list exceeds {max}");
    let mut seen = BTreeSet::new();
    for value in values {
        super::constraints::id(value)?;
        ensure!(seen.insert(value), "Duplicate agent identifier: {value}");
    }
    Ok(())
}
fn text(value: &str, max: usize) -> Result<()> {
    ensure!(
        !value.trim().is_empty() && value.len() <= max && !value.chars().any(char::is_control),
        "Agent instruction text must be nonempty, bounded and contain no control characters"
    );
    Ok(())
}
fn texts(values: &[String]) -> Result<()> {
    ensure!(
        values.len() <= 16,
        "Agent instructions exceed 16 entries per polarity"
    );
    let mut seen = BTreeSet::new();
    for value in values {
        text(value, 4096)?;
        ensure!(seen.insert(value.trim()), "Duplicate agent instruction");
    }
    Ok(())
}

pub fn schema() -> Result<serde_json::Value> {
    let value = serde_json::from_str(include_str!(
        "../../../skills/qualitygate-cli/references/schemas/agent-obligation.schema.json"
    ))?;
    jsonschema::draft202012::new(&value)?;
    Ok(value)
}
