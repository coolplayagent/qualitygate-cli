use super::*;
use anyhow::{Result, bail};
use std::collections::BTreeSet;
use std::path::Path;

pub(super) fn validate(config: &Config) -> Result<()> {
    layout(config, true)
}

pub(super) fn layout(config: &Config, resolved: bool) -> Result<()> {
    if config.schema_version != 1 {
        bail!(
            "Unsupported configuration schema_version: {}",
            config.schema_version
        );
    }
    let mut ids: BTreeSet<&str> = config.rules.keys().map(String::as_str).collect();
    let mut packages = BTreeSet::new();
    for package in &config.rulesets {
        if !RULESETS.contains(&package.as_str()) || !packages.insert(package) {
            bail!("Unknown or duplicate ruleset: {package}");
        }
    }
    for (id, rule) in &config.rules {
        validate_id(id)?;
        if let Some(source) = &rule.source {
            constraints::source(source)?;
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
        let mut outputs = BTreeSet::new();
        let mut roots = BTreeSet::new();
        for project in &check.projects {
            let root = crate::paths::relative(Path::new(&project.root))?;
            if project.root.is_empty()
                || (project.root != "." && root != project.root)
                || !roots.insert(root)
            {
                bail!("Maven project roots must be unique and normalized");
            }
            for path in [&project.effective_pom, &project.dependency_tree] {
                let normalized = crate::paths::relative(Path::new(path))?;
                if normalized.is_empty() || normalized != *path || !outputs.insert(path) {
                    bail!("Maven output paths must be distinct normalized files");
                }
            }
        }
        if !check.projects.is_empty()
            && (check.kind != CheckKind::Command
                || check.expected_exit_code != 0
                || !check.findings_exit_codes.is_empty()
                || check
                    .reports
                    .iter()
                    .any(|report| report.mode != IncrementMode::Full))
        {
            bail!("Maven project facts require successful commands and full reports");
        }
        let mut tools = BTreeSet::new();
        for tool in &check.tools {
            validate_id(&tool.id)?;
            if !(1..=60).contains(&tool.timeout_seconds) {
                bail!("Tool version timeout_seconds must be 1..60: {}", tool.id);
            }
            if !tools.insert(&tool.id) || tool.argv.is_empty() || tool.argv[0].trim().is_empty() {
                bail!(
                    "Tool version probes need unique IDs and nonempty argv: {}",
                    check.id
                );
            }
            for path in &tool.inputs {
                crate::paths::relative(Path::new(path))?;
            }
        }
        let mut findings = BTreeSet::new();
        for code in &check.findings_exit_codes {
            if check.reports.is_empty()
                || *code == check.expected_exit_code
                || !findings.insert(code)
            {
                bail!(
                    "findings_exit_codes needs reports and distinct non-success codes: {}",
                    check.id
                );
            }
        }
        for report in &check.reports {
            if !outputs.insert(&report.path) {
                bail!("Check output paths must be distinct");
            }
            crate::paths::relative(Path::new(&report.path))?;
            if let Some(baseline) = &report.baseline {
                crate::paths::relative(Path::new(baseline))?;
            }
            if report.minimum_tests == Some(0) {
                bail!("minimum_tests must be at least one");
            }
            if [
                ReportFormat::Lcov,
                ReportFormat::Cobertura,
                ReportFormat::Jacoco,
            ]
            .contains(&report.format)
                || report.minimum_coverage.is_some()
                || !report.coverage_paths.is_empty()
            {
                if report.coverage_paths.is_empty() {
                    bail!("Coverage requires explicit coverage_paths for its source inventory");
                }
                for path in &report.coverage_paths {
                    globset::Glob::new(path)?;
                }
                if ![IncrementMode::Full, IncrementMode::ChangedLines].contains(&report.mode) {
                    bail!("Coverage supports full or changed_lines mode");
                }
            }
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
        if name == "full" && resolved {
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
    let nodes: Vec<_> = config
        .rules
        .iter()
        .map(|(id, rule)| (id, &rule.depends_on))
        .chain(
            config
                .checks
                .iter()
                .map(|check| (&check.id, &check.depends_on)),
        )
        .collect();
    let mut completed = BTreeSet::new();
    for _ in 0..=nodes.len() {
        for (id, dependencies) in &nodes {
            let mut unique = BTreeSet::new();
            for dependency in *dependencies {
                if !ids.contains(dependency.as_str()) {
                    bail!("Unknown prerequisite: {dependency}");
                }
                if !unique.insert(dependency) {
                    bail!("Repeated prerequisite: {dependency}");
                }
            }
            if dependencies.iter().all(|id| completed.contains(id)) {
                completed.insert((*id).clone());
            }
        }
    }
    if completed.len() != nodes.len() {
        bail!("Check dependency graph contains a cycle");
    }
    if let Some(path) = &config.custom_rules {
        let normalized = crate::paths::relative(Path::new(path))?;
        if normalized == "." || normalized.is_empty() || normalized != path.trim_end_matches('/') {
            bail!("custom_rules must be a normalized repository subdirectory");
        }
    }
    for path in &config.verification_assets {
        globset::Glob::new(path)?;
    }
    Ok(())
}

pub(super) fn validate_id(id: &str) -> Result<()> {
    constraints::id(id)
}
