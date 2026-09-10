use super::{Gap, Project, Suggestion, inventory::Inventory, suggestions};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

type Analysis = (Vec<&'static str>, Vec<Suggestion>, Vec<Gap>);

fn gap(project: &Project, capability: &str, reason: &str) -> Gap {
    Gap {
        path: project.manifest.clone(),
        capability: capability.into(),
        reason: reason.into(),
    }
}

fn pytest_requirement(value: &str) -> bool {
    // Discovery only recognizes a framework declaration; resolution remains the
    // responsibility of the project adapter and the actual interpreter.
    value.len() <= 8192
        && value.bytes().filter(|byte| *byte == b'(').count() <= 64
        && value
            .parse::<pep508_rs::Requirement<reqwest::Url>>()
            .is_ok_and(|requirement| requirement.name.as_ref() == "pytest")
}

pub(super) fn analyze(inventory: &Inventory, project: &Project, text: &str) -> Result<Analysis> {
    let manifest = &project.manifest;
    let root = &project.root;
    let mut gaps = Vec::new();
    let (capabilities, checks) = match project.ecosystem.as_str() {
        "cargo" => {
            let data: toml::Value =
                toml::from_str(text).map_err(|_| anyhow::anyhow!("Invalid Cargo TOML"))?;
            if data.get("package").is_none() && data.get("workspace").is_none() {
                bail!("Cargo manifest needs package or workspace metadata")
            }
            let workspace = data.get("workspace").is_some();
            gaps.push(gap(project, "project_semantics", "Cargo syntax and test summaries are supported; Cargo dependency ownership/module-boundary facts need another adapter or tool"));
            (
                vec![],
                suggestions::cargo(inventory, manifest, root, workspace)?,
            )
        }
        "maven" => {
            let data = roxmltree::Document::parse(text)
                .map_err(|_| anyhow::anyhow!("Invalid Maven XML"))?;
            if data.root_element().tag_name().name() != "project" {
                bail!("Maven manifest root must be project")
            }
            gaps.push(gap(project, "project_facts_configuration", "Configure effective-POM/dependency-tree producer outputs before dependency assertions; compiled usage and module directions need their explicit producer contracts"));
            (
                vec![
                    "dependency_resolution",
                    "used_undeclared",
                    "module_dependency_directions",
                ],
                suggestions::maven(inventory, manifest, root)?,
            )
        }
        "python" => {
            let mut has_pytest = false;
            let mut capabilities = Vec::new();
            if manifest.ends_with("pyproject.toml") {
                let data: toml::Value =
                    toml::from_str(text).map_err(|_| anyhow::anyhow!("Invalid Python TOML"))?;
                let project_data = data.get("project");
                let static_metadata = project_data.is_some_and(|project| {
                    project.get("name").and_then(toml::Value::as_str).is_some()
                        && project
                            .get("version")
                            .and_then(toml::Value::as_str)
                            .is_some()
                        && project
                            .get("dynamic")
                            .is_none_or(|dynamic| dynamic.as_array().is_some_and(Vec::is_empty))
                });
                if static_metadata {
                    capabilities.push("dependency_resolution");
                    gaps.push(gap(project, "project_facts_configuration", "Configure source/test roots, extras, a fresh pip install target/report and version probes to produce Python facts"));
                } else {
                    gaps.push(gap(project, "project_semantics", "Python facts need static PEP 621 name/version metadata; dynamic, legacy and Poetry-only metadata are not yet supported"));
                }
                has_pytest = data
                    .get("tool")
                    .and_then(|tool| tool.get("pytest"))
                    .is_some();
                if let Some(project_data) = project_data {
                    for requirements in project_data.get("dependencies").into_iter().chain(
                        project_data
                            .get("optional-dependencies")
                            .and_then(toml::Value::as_table)
                            .into_iter()
                            .flat_map(|table| table.values()),
                    ) {
                        if let Some(requirements) = requirements.as_array() {
                            has_pytest |= requirements
                                .iter()
                                .filter_map(toml::Value::as_str)
                                .any(pytest_requirement);
                        }
                    }
                }
            } else if manifest.ends_with("requirements.txt") {
                has_pytest = text.lines().any(|line| pytest_requirement(line.trim()));
                gaps.push(gap(project, "project_semantics", "Requirements files describe environment inputs; static PEP 621 metadata is required by the Python project-facts adapter"));
            } else {
                gaps.push(gap(project, "project_semantics", "Legacy Python setup files are detected without executing them; migrate metadata or configure existing build/test commands"));
            }
            if !has_pytest {
                gaps.push(gap(project, "test_command", "No pytest declaration/configuration found; select the repository's existing test command and a test-count report"));
            }
            let checks = if has_pytest {
                vec![suggestions::pytest(manifest, root)?]
            } else {
                vec![]
            };
            (capabilities, checks)
        }
        "node" => {
            let data: Value = serde_json::from_str(text).context("Invalid package.json")?;
            let object = data.as_object().context("package.json must be an object")?;
            let mut managers = std::collections::BTreeSet::new();
            for (file, manager) in [
                ("package-lock.json", "npm"),
                ("npm-shrinkwrap.json", "npm"),
                ("pnpm-lock.yaml", "pnpm"),
                ("yarn.lock", "yarn"),
                ("bun.lock", "bun"),
                ("bun.lockb", "bun"),
            ] {
                if inventory.files.contains(&suggestions::joined(root, file)) {
                    managers.insert(manager);
                }
            }
            if let Some(manager) = object.get("packageManager").and_then(Value::as_str) {
                let manager = manager.split('@').next().unwrap_or_default();
                if !["npm", "pnpm", "yarn", "bun"].contains(&manager) {
                    bail!("Unsupported packageManager; choose an explicit launcher");
                }
                managers.insert(manager);
            }
            if managers.len() > 1 {
                bail!("Conflicting package manager declarations/lockfiles")
            }
            let manager = managers.first().copied().unwrap_or("npm");
            let scripts = object
                .get("scripts")
                .map(|value| {
                    value
                        .as_object()
                        .context("package.json scripts must be an object")
                })
                .transpose()?;
            let checks = scripts
                .map(|scripts| suggestions::node(manifest, root, manager, scripts))
                .transpose()?
                .unwrap_or_default();
            gaps.push(gap(project, "project_semantics", "JavaScript/TypeScript syntax is supported; package resolution/import ownership and test-reporter mapping require configured ecosystem tools"));
            if checks.is_empty() {
                gaps.push(gap(
                    project,
                    "verification_command",
                    "No nonempty build/lint/test package script found",
                ))
            }
            (vec![], checks)
        }
        "go" => {
            if !text
                .lines()
                .any(|line| line.trim_start().starts_with("module "))
            {
                bail!("go.mod has no module declaration")
            }
            let mut checks = Vec::new();
            for goal in ["build", "test"] {
                checks.push(suggestions::command(manifest, root, &format!("go-{goal}"),
                    vec!["go".into(),goal.into(),"./...".into()],vec![json!({"id":"go","argv":["go","version"]})],
                    if goal == "test" { "report_mapping_required" } else { "ready_for_review" },
                    vec![if goal == "test" { "Convert actual go test results to a supported test-count report and configure minimum_tests" } else { "Review build tags and module/workspace selection" }.into()])?);
            }
            gaps.push(gap(project, "project_semantics", "Go syntax is supported; resolved import ownership and dependency direction facts need configured ecosystem tools"));
            (vec![], checks)
        }
        _ => {
            gaps.push(gap(project, "project_semantics", "Manifest detected, but this ecosystem's metadata and command conventions need an explicit adapter or team configuration"));
            (vec![], vec![])
        }
    };
    Ok((capabilities, checks, gaps))
}
