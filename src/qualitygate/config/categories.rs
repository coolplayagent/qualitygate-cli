//! Mutable discovery labels, independent of rule activation and package selection.

use super::{Config, catalog::Entry};
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Category {
    pub description: String,
    /// Stable starter identity, preserved on rename; absent for custom labels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
}

pub fn defaults() -> BTreeMap<String, Category> {
    [
        ("core", "Commit, diff, and file format rules"),
        ("test", "Test naming, structure, and traceability rules"),
        ("security", "Security-sensitive APIs and patterns"),
        ("architecture", "Import boundaries and module dependencies"),
        ("style", "Comment language and code style"),
        ("project", "Project-authored rule definitions"),
    ]
    .into_iter()
    .map(|(name, description)| {
        (
            name.into(),
            Category {
                description: description.into(),
                origin: Some(name.into()),
            },
        )
    })
    .collect()
}

pub fn registry(config: &Config) -> BTreeMap<String, Category> {
    config.categories.clone().unwrap_or_else(defaults)
}

pub fn validate(config: &Config) -> Result<()> {
    let categories = registry(config);
    if categories.len() > 256 || config.rule_categories.len() > 4096 {
        bail!("Category inventory exceeds 256 categories or 4096 assignments");
    }
    let defaults = defaults();
    let mut origins = BTreeSet::new();
    for (name, category) in &categories {
        super::constraints::id(name)?;
        if category.description.len() > 1024 || category.description.chars().any(char::is_control) {
            bail!("Category descriptions must be at most 1024 bytes without control characters");
        }
        if let Some(origin) = &category.origin
            && (!defaults.contains_key(origin) || !origins.insert(origin))
        {
            bail!("Category origins must be distinct starter category identities");
        }
    }
    for (id, name) in &config.rule_categories {
        super::constraints::id(id)?;
        if !categories.contains_key(name) {
            bail!("Rule {id} references unknown category: {name}");
        }
    }
    Ok(())
}

fn origin(entry: &Entry) -> &str {
    let Some(rule) = &entry.builtin else {
        return "project";
    };
    match rule.implementation.as_str() {
        "line-ending" | "commit-message" | "diff-size" => "core",
        "test-naming" | "parameterized-tests" | "ai-code-traceability" => "test",
        "source-pattern" if rule.id != "todo-marker" => "security",
        "import-boundary" | "module-boundary" | "used-undeclared" => "architecture",
        _ => "style",
    }
}

pub fn assigned<'a>(
    config: &'a Config,
    categories: &'a BTreeMap<String, Category>,
    id: &str,
    entry: &Entry,
) -> Option<&'a str> {
    config
        .rule_categories
        .get(id)
        .map(String::as_str)
        .or_else(|| {
            categories
                .iter()
                .find(|(_, category)| category.origin.as_deref() == Some(origin(entry)))
                .map(|(name, _)| name.as_str())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_bounds_origins_and_assignments_are_strict() {
        for yaml in [
            "categories: {x: {description: fine, typo: true}}",
            "categories: {x: {description: fine, origin: foreign}}",
            "categories: {x: {description: fine, origin: core}, y: {description: fine, origin: core}}",
            "categories: {x: {description: fine}, x: {description: duplicate}}",
            "categories: {}\nrule_categories: {line-ending: missing}",
            "rule_categories: {line-ending: core, line-ending: test}",
            "rule_categories: {bad/id: core}",
        ] {
            let yaml = format!("schema_version: 1\n{yaml}");
            assert!(super::super::parse(yaml.as_bytes()).is_err(), "{yaml}");
        }
        let mut config = Config {
            categories: Some(
                (0..257)
                    .map(|n| {
                        (
                            format!("category-{n}"),
                            Category {
                                description: String::new(),
                                origin: None,
                            },
                        )
                    })
                    .collect(),
            ),
            ..Config::default()
        };
        assert!(validate(&config).is_err());
        config.categories = None;
        config.rule_categories = (0..4097)
            .map(|n| (format!("rule-{n}"), "core".into()))
            .collect();
        assert!(validate(&config).is_err());
        config.rule_categories.clear();
        config.categories = Some(BTreeMap::from([(
            "long".into(),
            Category {
                description: "x".repeat(1025),
                origin: None,
            },
        )]));
        assert!(validate(&config).is_err());
    }
}
