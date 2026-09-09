//! Strict policy parameters for facts supplied by project adapters.

use super::RuleSetting;
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, path::Path};

pub const MAVEN_USAGE_GOAL: &str =
    "org.apache.maven.plugins:maven-dependency-plugin:3.8.1:analyze-only";
pub const MAVEN_USAGE_ARGS: &[&str] = &[
    "-N",
    "-Dverbose=false",
    "-DscriptableOutput=false",
    "-DoutputXML=false",
    "-DfailOnWarning=false",
    "-Dmdep.analyze.skip=false",
    "-Dmdep.analyze.excludedClasses=",
    "-Danalyzer=default",
    "-Dstyle.color=never",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UsedUndeclared {
    pub modules: Vec<String>,
}

pub fn used_undeclared(setting: &RuleSetting) -> Result<UsedUndeclared> {
    let policy: UsedUndeclared =
        serde_json::from_value(serde_json::to_value(&setting.parameters)?)?;
    if policy.modules.is_empty() || policy.modules.len() > 256 {
        bail!("used-undeclared requires 1..256 modules");
    }
    let mut roots = BTreeSet::new();
    for root in &policy.modules {
        let relative = crate::paths::relative(Path::new(root))?;
        if root.is_empty() || (root != "." && relative != *root) || !roots.insert(root) {
            bail!("used-undeclared modules must be unique normalized repository roots");
        }
    }
    Ok(policy)
}

pub(super) fn validate_usage_command(check: &super::CommandCheck) -> Result<()> {
    if !check.projects.iter().any(
        |project| matches!(project, super::ProjectSpec::Maven(project) if project.dependency_usage),
    ) {
        return Ok(());
    }
    if check.projects.len() != 1 || check.cwd != check.projects[0].root() {
        bail!("Maven dependency usage requires one project with cwd equal to its root");
    }
    for required in MAVEN_USAGE_ARGS {
        if !check.argv.iter().any(|arg| arg == required) {
            bail!("Maven dependency usage requires argument {required}");
        }
        if let Some((property, _)) = required.split_once('=') {
            let key = property.trim_start_matches("-D");
            let count = check
                .argv
                .iter()
                .filter_map(|arg| {
                    arg.strip_prefix("-D")
                        .or_else(|| arg.strip_prefix("--define="))
                })
                .filter(|arg| arg.split('=').next() == Some(key))
                .count();
            if count != 1 {
                bail!("Maven dependency usage has duplicate property {property}");
            }
        }
    }
    let goals: Vec<_> = check
        .argv
        .iter()
        .enumerate()
        .filter(|(_, arg)| ["clean", "test-compile", MAVEN_USAGE_GOAL].contains(&arg.as_str()))
        .collect();
    if goals.len() != 3
        || goals[0].1 != "clean"
        || goals[1].1 != "test-compile"
        || goals[2].1 != MAVEN_USAGE_GOAL
    {
        bail!(
            "Maven dependency usage requires ordered clean, test-compile and the pinned analyze-only goal"
        );
    }
    if check.argv.iter().any(|arg| {
        [
            "-q",
            "--quiet",
            "-l",
            "--log-file",
            "-T",
            "--threads",
            "-f",
            "--file",
            "-pl",
            "--projects",
            "-D",
            "--define",
        ]
        .contains(&arg.as_str())
            || arg.starts_with("--log-file=")
            || arg.starts_with("--threads=")
            || arg.starts_with("--file=")
            || arg.starts_with("--projects=")
    }) {
        bail!("Maven dependency usage requires an unredirected single-module log");
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleBoundary {
    pub modules: Vec<String>,
    pub forbidden: Vec<ForbiddenDependency>,
    #[serde(default)]
    pub dependency_kind: DependencyKind,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyKind {
    #[default]
    Declared,
    Resolved,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForbiddenDependency {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub scopes: Vec<String>,
}

pub fn module_boundary(setting: &RuleSetting) -> Result<ModuleBoundary> {
    let policy: ModuleBoundary =
        serde_json::from_value(serde_json::to_value(&setting.parameters)?)?;
    if policy.modules.is_empty()
        || policy.modules.len() > 256
        || policy.forbidden.is_empty()
        || policy.forbidden.len() > 256
    {
        bail!("module-boundary requires 1..256 modules and forbidden directions");
    }
    let mut roots = BTreeSet::new();
    for root in &policy.modules {
        let relative = crate::paths::relative(Path::new(root))?;
        if root.is_empty() || (root != "." && relative != *root) || !roots.insert(root) {
            bail!("module-boundary modules must be unique normalized repository roots");
        }
    }
    for direction in &policy.forbidden {
        for pattern in [&direction.from, &direction.to] {
            if pattern.len() > 256
                || pattern.split(':').count() != 2
                || pattern.split(':').any(str::is_empty)
            {
                bail!("Module patterns must use group:artifact globs without a version");
            }
            globset::Glob::new(pattern)?;
        }
        let mut scopes = BTreeSet::new();
        for scope in &direction.scopes {
            if !["compile", "provided", "runtime", "test", "system"].contains(&scope.as_str())
                || !scopes.insert(scope)
            {
                bail!("Forbidden dependency scopes must be unique Maven scopes");
            }
        }
    }
    Ok(policy)
}
