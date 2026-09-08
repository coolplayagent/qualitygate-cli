use super::*;
use anyhow::{Result, bail};
use std::collections::BTreeSet;
use std::path::Path;

pub(super) fn validate(config: &Config) -> Result<()> {
    if config.schema_version != 1 {
        bail!(
            "Unsupported configuration schema_version: {}",
            config.schema_version
        );
    }
    let mut ids: BTreeSet<&str> = config.rules.keys().map(String::as_str).collect();
    for (id, rule) in &config.rules {
        validate_id(id)?;
        if let Some(source) = &rule.source {
            crate::paths::relative(Path::new(&source.document))?;
        }
    }
    for check in &config.checks {
        validate_id(&check.id)?;
        if !ids.insert(&check.id) {
            bail!("Duplicate check id: {}", check.id);
        }
        if check.kind == CheckKind::Command
            && (check.argv.is_empty() || check.argv[0].trim().is_empty())
        {
            bail!("Command {} needs nonempty argv", check.id);
        }
        if check.timeout_seconds == 0 || check.timeout_seconds > 86_400 {
            bail!("Check {} timeout_seconds must be 1..86400", check.id);
        }
        crate::paths::relative(Path::new(&check.cwd))?;
        for report in &check.reports {
            crate::paths::relative(Path::new(&report.path))?;
            if report
                .minimum_coverage
                .is_some_and(|number| !number.is_finite() || !(0.0..=100.0).contains(&number))
            {
                bail!("Coverage threshold must be finite and within 0..100");
            }
            if report.mode == IncrementMode::NewDiagnostics && report.baseline.is_none() {
                bail!("new_diagnostics requires a baseline report");
            }
        }
    }
    for (name, profile) in &config.profiles {
        if !["quick", "full"].contains(&name.as_str()) {
            bail!("Unknown profile: {name}");
        }
        let mut included = BTreeSet::new();
        for id in &profile.include {
            if !ids.contains(id.as_str()) || !included.insert(id) {
                bail!("Unknown or repeated check {id} in profile {name}");
            }
        }
        if name == "full" {
            for (id, required) in config
                .rules
                .iter()
                .map(|(id, rule)| (id, rule.enabled && rule.required))
                .chain(
                    config
                        .checks
                        .iter()
                        .map(|check| (&check.id, check.required)),
                )
            {
                if required && !included.contains(id) {
                    bail!("full profile omits required check: {id}");
                }
            }
        }
    }
    let mut completed = BTreeSet::new();
    for _ in 0..=config.checks.len() {
        for check in &config.checks {
            for dependency in &check.depends_on {
                if !ids.contains(dependency.as_str()) {
                    bail!("Unknown prerequisite: {dependency}");
                }
            }
            if check
                .depends_on
                .iter()
                .all(|id| completed.contains(id) || config.rules.contains_key(id))
            {
                completed.insert(check.id.clone());
            }
        }
    }
    if completed.len() != config.checks.len() {
        bail!("Check dependency graph contains a cycle");
    }
    if let Some(path) = &config.custom_rules {
        crate::paths::relative(Path::new(path))?;
    }
    for path in &config.verification_assets {
        globset::Glob::new(path)?;
    }
    Ok(())
}

pub(super) fn validate_id(id: &str) -> Result<()> {
    if id.is_empty()
        || id.len() > 128
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        bail!("Invalid check id: {id}");
    }
    Ok(())
}
