use super::{Suggestion, inventory::Inventory};
use anyhow::Result;
use serde_json::json;

pub(super) fn joined(root: &str, name: &str) -> String {
    if root == "." {
        name.into()
    } else {
        format!("{root}/{name}")
    }
}

pub(super) fn command(
    manifest: &str,
    root: &str,
    purpose: &str,
    argv: Vec<String>,
    tools: Vec<serde_json::Value>,
    status: &'static str,
    review: Vec<String>,
) -> Result<Suggestion> {
    let hash = super::digest(manifest.as_bytes());
    let check = serde_json::from_value(json!({
        "id": format!("init-{purpose}-{}", &hash[7..19]), "argv": argv, "cwd": root,
        "timeout_seconds": 900, "tools": tools,
    }))?;
    Ok(Suggestion {
        purpose: purpose.into(),
        source: manifest.into(),
        status,
        review,
        check,
    })
}

pub(super) fn cargo(
    inventory: &Inventory,
    manifest: &str,
    root: &str,
    workspace: bool,
) -> Result<Vec<Suggestion>> {
    let mut output = Vec::new();
    for goal in ["check", "test"] {
        let mut argv = vec!["cargo".into(), goal.into()];
        if inventory.files.contains(&joined(root, "Cargo.lock")) {
            argv.push("--locked".into());
        }
        if workspace {
            argv.push("--workspace".into());
        }
        output.push(command(manifest, root, &format!("cargo-{goal}"), argv,
            vec![json!({"id":"cargo","argv":["cargo","--version"]}), json!({"id":"rustc","argv":["rustc","--version","--verbose"]})],
            "ready_for_review", vec![
                "Review feature selection and dependency lock policy for this project".into(),
                "Cargo test summaries must show at least one executed test; custom runners need explicit reports".into(),
            ])?);
    }
    Ok(output)
}

pub(super) fn maven(inventory: &Inventory, manifest: &str, root: &str) -> Result<Vec<Suggestion>> {
    let wrapper = if cfg!(windows) { "mvnw.cmd" } else { "mvnw" };
    let executable = if inventory.files.contains(&joined(root, wrapper)) {
        format!("./{wrapper}")
    } else {
        "mvn".into()
    };
    let mut base = vec![executable.clone(), "-B".into()];
    if inventory.files.contains(&joined(root, "settings.xml")) {
        base.extend(["-s".into(), "settings.xml".into()]);
    }
    let mut output = Vec::new();
    for goal in ["compile", "verify"] {
        let mut argv = base.clone();
        argv.push(goal.into());
        let mut suggestion = command(manifest, root, &format!("maven-{goal}"), argv,
            vec![json!({"id":"maven","argv":[executable,"--version"]})],
            if goal == "verify" { "report_mapping_required" } else { "ready_for_review" },
            vec![if goal == "verify" {
                "Map fresh Surefire/Failsafe JUnit files with minimum_tests; aggregate or enumerate all accepted test suites"
            } else {
                "This command compiles main sources; it does not establish successful tests"
            }.into()])?;
        if base.iter().any(|argument| argument == "-s") {
            suggestion.check.required_args = vec!["-s".into(), "settings.xml".into()];
        }
        output.push(suggestion);
    }
    Ok(output)
}

pub(super) fn pytest(manifest: &str, root: &str) -> Result<Suggestion> {
    let python = if cfg!(windows) { "python" } else { "python3" };
    let mut suggestion = command(manifest, root, "pytest",
        vec![python.into(), "-E".into(), "-P".into(), "-m".into(), "pytest".into(), "--junitxml=target/qualitygate-junit.xml".into()],
        vec![
            json!({"id":"python","argv":[python,"-E","-P","--version"]}),
            json!({"id":"pytest","argv":[python,"-E","-P","-m","pytest","--version"]}),
        ], "ready_for_review", vec![
            "Select a Python 3.11+ interpreter with the project and declared test dependencies installed".into(),
            "Review pytest configuration and import paths; installation/project-fact commands require separate configuration".into(),
        ])?;
    suggestion.check.reports.push(serde_json::from_value(json!({
        "path":joined(root, "target/qualitygate-junit.xml"),"format":"junit","minimum_tests":1,
    }))?);
    suggestion.check.findings_exit_codes = vec![1];
    Ok(suggestion)
}

pub(super) fn node(
    manifest: &str,
    root: &str,
    manager: &str,
    scripts: &serde_json::Map<String, serde_json::Value>,
) -> Result<Vec<Suggestion>> {
    let mut result = Vec::new();
    for name in ["build", "lint", "test"] {
        if !scripts.get(name).is_some_and(|value| {
            value
                .as_str()
                .is_some_and(|script| !script.trim().is_empty())
        }) {
            continue;
        }
        let tools = if manager == "bun" {
            vec![json!({"id":"bun","argv":["bun","--version"]})]
        } else {
            vec![
                json!({"id":"node","argv":["node","--version"]}),
                json!({"id":"package-manager","argv":[manager,"--version"]}),
            ]
        };
        let mut suggestion = command(manifest, root, &format!("{manager}-{name}"),
            vec![manager.into(),"run".into(),name.into()],tools,
            if name == "test" { "report_mapping_required" } else { "ready_for_review" },
            vec![if name == "test" {
                "Configure the framework's JUnit reporter, target test inventory, minimum_tests and assertion-failure exit codes"
            } else {
                "Review the existing package script; its exit status does not provide normalized incremental diagnostics"
            }.into()])?;
        suggestion.source = format!("{manifest}#scripts.{name}");
        result.push(suggestion);
    }
    Ok(result)
}
