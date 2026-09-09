//! Dependency Plugin 3.8.1 bytecode-analysis logs, cross-checked with resolved facts.
//! An empty/unknown/skipped log is missing evidence, never an empty finding set.

use crate::{
    domain::{DependencyFact, DependencyUsage, ProjectFacts},
    snapshot::Snapshot,
};
use anyhow::{Context, Result, bail};
use std::collections::BTreeSet;
use std::time::{Duration, Instant};

#[cfg(test)]
#[path = "maven_usage_tests.rs"]
mod tests;

pub fn parse(
    model: &[u8],
    log: &[u8],
    facts: &ProjectFacts,
    snapshot: &Snapshot,
) -> Result<DependencyUsage> {
    if log.len() > 16 * 1024 * 1024 || model.len() > crate::snapshot::MAX_FILE_BYTES {
        bail!("Maven usage log exceeds analysis budget");
    }
    validate_model(model)?;
    let log = std::str::from_utf8(log)?;
    if log.contains('\0') || log.contains('\u{1b}') {
        bail!("Maven usage requires plain UTF-8 logs");
    }
    let artifact = facts
        .coordinate
        .split(':')
        .nth(1)
        .context("Invalid Maven project identity")?;
    let header = format!("--- dependency:3.8.1:analyze-only (default-cli) @ {artifact} ---");
    let compilation = regex::Regex::new(r"^Compiling ([1-9][0-9]*) source files?(?: |$)")?;
    let mut stage = None;
    let mut compiled = [0usize; 2];
    let mut clean = false;
    let mut analyzing = false;
    let mut analyzed = false;
    let mut success = false;
    let mut clean_result = false;
    let mut section: Option<&str> = None;
    let mut sections = BTreeSet::new();
    let mut section_count = 0;
    let mut missing = BTreeSet::new();
    let started = Instant::now();
    for line in log.lines() {
        if started.elapsed() > Duration::from_secs(30) {
            bail!("Maven usage analysis exceeded 30 seconds");
        }
        let Some(body) = line
            .strip_prefix("[INFO] ")
            .or_else(|| line.strip_prefix("[WARNING] "))
        else {
            if analyzing && !line.trim().is_empty() {
                bail!("Unrecognized Maven analysis output: {line}");
            }
            continue;
        };
        if body.starts_with("--- ") {
            if analyzed {
                bail!("Maven analyze-only must be the final single-module goal");
            }
            stage = None;
            if body.contains(":clean (default-clean)") {
                if clean || compiled != [0, 0] {
                    bail!("Repeated or late Maven clean invalidates compilation evidence");
                }
                clean = true;
            }
            if body.contains(":compile (default-compile)") {
                stage = Some(0);
            }
            if body.contains(":testCompile (default-testCompile)") {
                stage = Some(1);
            }
            if body.contains(":analyze") {
                if body != header || !clean {
                    bail!(
                        "Unsupported Maven analysis version, module, execution or missing clean phase"
                    );
                }
                analyzed = true;
                analyzing = true;
            }
            continue;
        }
        if !analyzed {
            if let (Some(stage), Some(capture)) = (stage, compilation.captures(body)) {
                if !clean {
                    bail!("Compilation must follow clean");
                }
                compiled[stage] = compiled[stage]
                    .checked_add(capture[1].parse()?)
                    .context("Compilation count overflow")?;
            }
            continue;
        }
        if body == "BUILD SUCCESS" {
            success = true;
        }
        if body.starts_with("-----") {
            if section.is_some() && section_count == 0 {
                bail!("Empty Maven dependency section");
            }
            analyzing = false;
            continue;
        }
        if !analyzing || body.trim().is_empty() {
            continue;
        }
        if body == "No dependency problems found" {
            if clean_result || !sections.is_empty() {
                bail!("Contradictory Maven analysis result");
            }
            clean_result = true;
        } else if [
            "Used undeclared dependencies found:",
            "Unused declared dependencies found:",
            "Non-test scoped test only dependencies found:",
        ]
        .contains(&body)
        {
            if clean_result || !sections.insert(body) || (section.is_some() && section_count == 0) {
                bail!("Duplicate, empty or contradictory Maven analysis sections");
            }
            section = Some(body);
            section_count = 0;
        } else if let Some(section) = section {
            let dependency = coordinate(body.trim())?;
            if !facts.resolved.contains(&dependency) {
                bail!("Maven usage artifact is absent from the resolved project tree");
            }
            let declared = facts.declared.contains(&dependency);
            if section == "Used undeclared dependencies found:" {
                if declared || !missing.insert(dependency) {
                    bail!("Contradictory or duplicate undeclared Maven dependency");
                }
            } else if !declared {
                bail!("Declared Maven usage artifact has no direct declaration");
            }
            section_count += 1;
        } else {
            bail!("Maven analysis did not provide a recognized complete result: {body}");
        }
    }
    if !analyzed || !success || analyzing || (!clean_result && sections.is_empty()) {
        bail!("Maven dependency analysis was absent, skipped, truncated or unsuccessful");
    }
    let expected = [&facts.source_root, &facts.test_source_root].map(|root| {
        snapshot
            .files
            .keys()
            .filter(|path| {
                path.ends_with(".java")
                    && path
                        .strip_prefix(root)
                        .is_some_and(|rest| rest.starts_with('/'))
            })
            .count()
    });
    if expected == [0, 0] || expected != compiled {
        bail!(
            "Maven compilation inventory differs from snapshot Java sources: expected {expected:?}, compiled {compiled:?}"
        );
    }
    Ok(DependencyUsage {
        analyzer: "maven-dependency-plugin:3.8.1/default".into(),
        compiled_main_sources: compiled[0],
        compiled_test_sources: compiled[1],
        used_undeclared: missing.into_iter().collect(),
    })
}

fn coordinate(value: &str) -> Result<DependencyFact> {
    let parts: Vec<_> = value.split(':').collect();
    if !(5..=6).contains(&parts.len())
        || parts.iter().any(|part| {
            part.is_empty() || part.contains("${") || part.chars().any(char::is_whitespace)
        })
    {
        bail!("Malformed Maven analysis artifact: {value}");
    }
    let end = parts.len();
    if !["compile", "provided", "runtime", "test", "system"].contains(&parts[end - 1]) {
        bail!("Unsupported Maven analysis scope");
    }
    Ok(DependencyFact {
        group: parts[0].into(),
        artifact: parts[1].into(),
        artifact_type: parts[2].into(),
        classifier: if end == 6 {
            parts[3].into()
        } else {
            String::new()
        },
        version: parts[end - 2].into(),
        scope: parts[end - 1].into(),
    })
}

fn validate_model(model: &[u8]) -> Result<()> {
    let model = roxmltree::Document::parse(std::str::from_utf8(model)?)?;
    for plugin in model
        .descendants()
        .filter(|node| node.has_tag_name("plugin"))
    {
        let artifact = plugin
            .children()
            .find(|node| node.has_tag_name("artifactId"))
            .and_then(|node| node.text());
        if !matches!(
            artifact,
            Some("maven-dependency-plugin" | "maven-compiler-plugin")
        ) {
            continue;
        }
        if artifact == Some("maven-dependency-plugin")
            && plugin
                .children()
                .any(|node| node.has_tag_name("dependencies"))
        {
            bail!("Custom Maven analyzer dependencies are not supported");
        }
        for config in plugin
            .descendants()
            .filter(|node| node.has_tag_name("configuration"))
        {
            for parameter in config.children().filter(|node| node.is_element()) {
                let name = parameter.tag_name().name();
                if artifact == Some("maven-dependency-plugin") {
                    let expected = match name {
                        "verbose" | "scriptableOutput" | "outputXML" | "failOnWarning" | "skip" => {
                            "false"
                        }
                        "analyzer" => "default",
                        _ => {
                            bail!("Unsupported Maven analysis configuration may hide usage: {name}")
                        }
                    };
                    if parameter.text().map(str::trim) != Some(expected) {
                        bail!(
                            "Maven analysis configuration conflicts with evidence contract: {name}"
                        );
                    }
                } else if matches!(
                    name,
                    "skip"
                        | "skipMain"
                        | "skipMultiThreadWarning"
                        | "skipTests"
                        | "includes"
                        | "testIncludes"
                        | "excludes"
                        | "testExcludes"
                        | "compileSourceRoots"
                        | "outputDirectory"
                        | "testOutputDirectory"
                        | "compilerId"
                        | "compilerArgs"
                        | "compilerArguments"
                        | "executable"
                ) {
                    bail!(
                        "Unsupported Maven compiler configuration prevents complete source accounting: {name}"
                    );
                }
            }
        }
    }
    Ok(())
}
