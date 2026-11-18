//! Real Pyright PEP 702 diagnostics from unchanged users of newly deprecated APIs.
mod common;

use common::{cli, fixture, git, report};
use serde_json::{Value, json};
use std::{fs, path::Path};

const POLICY: &str = include_str!("../skills/qualitygate-cli/references/python-deprecation.yaml");
const PYRIGHT_CONFIG: &str =
    include_str!("../skills/qualitygate-cli/references/python-deprecation-pyrightconfig.json");
const CONFIG_PATH: &str = "pyright-deprecation.json";
const LIBRARY: &str = "src/library.py";
const CONSUMER: &str = "src/app.py";
const OLD_API: &str = r#"from warnings import deprecated

@deprecated("Use current_function instead")
def legacy_function() -> int:
    return 1

@deprecated("Use CurrentType instead")
class LegacyType:
    pass
"#;
const CONSUMER_SOURCE: &str = r#"from src import library

value = library.legacy_function()
item = library.LegacyType()
"#;

#[test]
fn python_deprecation_reference_policy_is_valid() {
    let fixture = fixture();
    let root = fixture.path();
    fs::write(root.join("qualitygate.yaml"), POLICY).unwrap();
    fs::write(root.join(CONFIG_PATH), PYRIGHT_CONFIG).unwrap();
    let output = cli(root, &["config", "--show", "--format", "json"]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn check(root: &Path, exit: i32) -> Value {
    report(
        &cli(
            root,
            &[
                "check",
                "--worktree",
                "--profile",
                "full",
                "--format",
                "json",
            ],
        ),
        exit,
    )
}

fn deprecation_result(report: &Value) -> &Value {
    report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["id"] == "python-deprecation")
        .unwrap()
}

#[test]
#[ignore = "requires Pyright 1.1.414; mandatory live Python deprecation CI job"]
fn real_pyright_reports_deprecated_classes_and_functions_and_incomplete_analysis() {
    let fixture = fixture();
    let root = fixture.path();
    fs::create_dir(root.join("src")).unwrap();
    fs::write(root.join("src/__init__.py"), "").unwrap();
    fs::write(root.join("qualitygate.yaml"), POLICY).unwrap();
    fs::write(root.join(CONFIG_PATH), PYRIGHT_CONFIG).unwrap();
    fs::write(root.join(CONSUMER), CONSUMER_SOURCE).unwrap();
    let current_api = OLD_API
        .lines()
        .filter(|line| !line.starts_with("@deprecated"))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(root.join(LIBRARY), &current_api).unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "Current library and its consumer"]);
    assert_eq!(deprecation_result(&check(root, 0))["verdict"], "pass");

    fs::write(root.join(LIBRARY), OLD_API).unwrap();
    let report = check(root, 1);
    assert_eq!(report["selection"]["changed_files"], json!([LIBRARY]));
    let result = deprecation_result(&report);
    assert_eq!(result["verdict"], "fail");
    let mut lines = result["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|diagnostic| {
            assert_eq!(diagnostic["file"], CONSUMER);
            assert_eq!(diagnostic["evidence"]["tool"], "pyright");
            assert_eq!(diagnostic["evidence"]["rule"], "reportDeprecated");
            diagnostic["range"]["start_line"].as_u64().unwrap()
        })
        .collect::<Vec<_>>();
    lines.sort_unstable();
    assert_eq!(lines, [3, 4]);

    fs::write(root.join(LIBRARY), &current_api).unwrap();
    assert_eq!(deprecation_result(&check(root, 0))["verdict"], "pass");

    fs::write(
        root.join(CONSUMER),
        format!("{CONSUMER_SOURCE}\nbroken: int = \"wrong\"\n"),
    )
    .unwrap();
    let report = check(root, 2);
    assert_eq!(report["gate"]["decision"], "incomplete");
    assert!(deprecation_result(&report)["verdict"].is_null());

    fs::write(root.join(CONSUMER), CONSUMER_SOURCE).unwrap();
    fs::write(
        root.join(CONFIG_PATH),
        PYRIGHT_CONFIG.replace("\"src\"", "\"absent\""),
    )
    .unwrap();
    let report = check(root, 2);
    assert_eq!(report["gate"]["decision"], "incomplete");
    assert!(deprecation_result(&report)["verdict"].is_null());
}
