//! Strict policy parameters for facts supplied by project adapters.

use super::RuleSetting;
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, path::Path};

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
