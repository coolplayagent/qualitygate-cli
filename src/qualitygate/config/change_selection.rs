//! Explicit command triggers and dependency closure over immutable change facts.
use super::{CheckKind, CommandCheck, Plan};
use crate::domain::change_selection::{ChangedInput, Selection};
use anyhow::{Result, bail};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn validate(check: &CommandCheck) -> Result<()> {
    let Some(when) = &check.when_changed else {
        return Ok(());
    };
    if check.kind != CheckKind::Command {
        bail!("when_changed requires a command check: {}", check.id);
    }
    if when.languages.is_empty() && when.paths.is_empty() && when.inputs.is_empty() {
        bail!(
            "when_changed needs languages, paths or inputs: {}",
            check.id
        );
    }
    let mut seen = BTreeSet::new();
    for language in &when.languages {
        if !(crate::domain::language::named(language).is_some()
            || matches!(language.as_str(), "c" | "cpp"))
            || !seen.insert(language)
        {
            bail!("Unknown or duplicate when_changed language: {language}");
        }
    }
    super::exclusions::matcher(&when.paths)?;
    super::exclusions::matcher(&when.inputs)?;
    Ok(())
}

/// Shared build/dependency inputs conservatively invalidate all scoped commands.
/// Repository-specific inputs belong in `when_changed.inputs` or verification_assets.
pub fn shared_build_input(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    matches!(
        name,
        "Cargo.toml"
            | "Cargo.lock"
            | "build.rs"
            | "rust-toolchain"
            | "rust-toolchain.toml"
            | "rustfmt.toml"
            | ".rustfmt.toml"
            | "clippy.toml"
            | ".clippy.toml"
            | "pom.xml"
            | "build.gradle"
            | "build.gradle.kts"
            | "settings.gradle"
            | "settings.gradle.kts"
            | "gradle.properties"
            | "gradle.lockfile"
            | "libs.versions.toml"
            | "gradle-wrapper.properties"
            | "pyproject.toml"
            | "setup.py"
            | "setup.cfg"
            | "Pipfile"
            | "Pipfile.lock"
            | "poetry.lock"
            | "uv.lock"
            | "tox.ini"
            | "pytest.ini"
            | "ruff.toml"
            | ".ruff.toml"
            | "mypy.ini"
            | ".mypy.ini"
            | ".flake8"
            | "package.json"
            | "package-lock.json"
            | "npm-shrinkwrap.json"
            | "yarn.lock"
            | "pnpm-lock.yaml"
            | "pnpm-workspace.yaml"
            | ".npmrc"
            | ".yarnrc.yml"
            | ".eslintignore"
            | ".prettierignore"
            | "go.mod"
            | "go.sum"
            | "go.work"
            | "go.work.sum"
            | ".golangci.yml"
            | ".golangci.yaml"
            | ".golangci.toml"
            | ".golangci.json"
            | "CMakeLists.txt"
            | "CMakePresets.json"
            | "Makefile"
            | "makefile"
            | ".clang-format"
            | ".clang-tidy"
            | "BUILD"
            | "BUILD.bazel"
            | "WORKSPACE"
            | "WORKSPACE.bazel"
            | "MODULE.bazel"
            | "MODULE.bazel.lock"
            | ".bazelrc"
            | ".bazelversion"
    ) || (name.starts_with("requirements") && name.ends_with(".txt"))
        || (name.starts_with("tsconfig") && name.ends_with(".json"))
        || name.starts_with("eslint.config.")
        || name == ".eslintrc"
        || name.starts_with(".eslintrc.")
        || name.starts_with("prettier.config.")
        || name == ".prettierrc"
        || name.starts_with(".prettierrc.")
        || name.ends_with(".bzl")
        || name.ends_with(".cmake")
        || matches!(path, ".cargo/config" | ".cargo/config.toml")
        || path.ends_with("/.cargo/config")
        || path.ends_with("/.cargo/config.toml")
}

pub fn select(
    plan: &Plan,
    changes: &[ChangedInput],
    force_reason: Option<&str>,
) -> Result<BTreeMap<String, Selection>> {
    let mut selections = BTreeMap::new();
    for command in &plan.commands {
        validate(command)?;
        let selection = if let Some(reason) = force_reason {
            Selection::new(true, reason)
        } else if let Some(when) = &command.when_changed {
            let paths = super::exclusions::matcher(&when.paths)?;
            let inputs = super::exclusions::matcher(&when.inputs)?;
            let matched = changes.iter().find(|change| {
                inputs.is_match(&change.path)
                    || ((!when.languages.is_empty() || !when.paths.is_empty())
                        && (when.paths.is_empty() || paths.is_match(&change.path))
                        && (when.languages.is_empty()
                            || change
                                .language
                                .as_ref()
                                .is_some_and(|language| when.languages.contains(language))))
            });
            let mut selection = Selection::new(
                matched.is_some(),
                if matched.is_some() {
                    "Changed input matches when_changed"
                } else {
                    "No changed input matches when_changed in the complete delivery selection"
                },
            );
            selection.matched_path = matched.map(|change| change.path.clone());
            selection
        } else {
            Selection::new(
                true,
                "No when_changed condition; execution is unconditional",
            )
        };
        selections.insert(command.id.clone(), selection);
    }
    // Reverse topological order propagates every live consumer to all ancestors.
    // Rules remain live: their producers must supply real evidence even if the
    // producer's own language/path condition would otherwise exclude execution.
    let commands: BTreeMap<_, _> = plan
        .commands
        .iter()
        .map(|command| (command.id.as_str(), command))
        .collect();
    for id in plan.order.iter().rev() {
        let dependencies = if let Some(rule) = plan.rules.get(id) {
            &rule.depends_on
        } else if let Some(command) = commands.get(id.as_str()) {
            if !selections[id].execute {
                continue;
            }
            &command.depends_on
        } else {
            continue;
        };
        for dependency in dependencies {
            if let Some(selection) = selections.get_mut(dependency)
                && !selection.execute
            {
                *selection = Selection::new(true, "Required by a selected dependent check");
                selection.required_by = Some(id.clone());
            }
        }
    }
    Ok(selections)
}

#[cfg(test)]
#[path = "change_selection_tests.rs"]
mod tests;
