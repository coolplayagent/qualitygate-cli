//! Explicit, opt-in paired test execution; no runner or language guessing.

use super::{CheckKind, CommandCheck, IncrementMode, ReportFormat};
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestEffectivenessSpec {
    pub source_paths: Vec<String>,
    pub test_paths: Vec<String>,
    #[serde(default)]
    pub support_paths: Vec<String>,
    pub assertion_failure_types: Vec<String>,
}

pub(super) fn validate(check: &CommandCheck) -> Result<()> {
    let Some(spec) = &check.test_effectiveness else {
        return Ok(());
    };
    if check.kind != CheckKind::Command
        || check.compatibility.is_some()
        || !check.projects.is_empty()
        || check.tools.is_empty()
        || check.expected_exit_code != 0
        || check.findings_exit_codes.is_empty()
        || check.reports.len() != 1
    {
        bail!(
            "Test effectiveness requires a command, tool probes, failure exits and exactly one full JUnit report; project and compatibility mappings are unsupported"
        );
    }
    let report = &check.reports[0];
    if report.format != ReportFormat::Junit
        || report.mode != IncrementMode::Full
        || report.baseline.is_some()
        || report.minimum_coverage.is_some()
        || !report.coverage_paths.is_empty()
    {
        bail!(
            "Test effectiveness requires a full JUnit report without baseline or coverage settings"
        );
    }
    for (paths, required) in [
        (&spec.source_paths, true),
        (&spec.test_paths, true),
        (&spec.support_paths, false),
    ] {
        if (required && paths.is_empty()) || paths.len() > 256 {
            bail!(
                "Test effectiveness path groups need at most 256 patterns and nonempty source/test groups"
            );
        }
        let mut unique = BTreeSet::new();
        for path in paths {
            if path.len() > 1024
                || path.is_empty()
                || path.contains(['\\', ':', '\0', '\r', '\n'])
                || path.starts_with('/')
                || path
                    .split('/')
                    .any(|part| part.is_empty() || part == "." || part == "..")
                || !unique.insert(path)
            {
                bail!(
                    "Test effectiveness paths must be unique normalized repository-relative globs"
                );
            }
            globset::Glob::new(path)?;
        }
    }
    let mut types = BTreeSet::new();
    if spec.assertion_failure_types.is_empty() || spec.assertion_failure_types.len() > 256 {
        bail!("Test effectiveness requires 1..256 explicit assertion failure types");
    }
    for kind in &spec.assertion_failure_types {
        if kind.trim().is_empty()
            || kind.trim() != kind
            || kind.len() > 256
            || kind.chars().any(char::is_control)
            || !types.insert(kind)
        {
            bail!("Assertion failure types must be unique bounded nonempty exact names");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn command() -> Value {
        json!({"id":"proof","argv":["runner"],"findings_exit_codes":[1],
            "tools":[{"id":"runner","argv":["runner","--version"]}],
            "reports":[{"path":"result.xml","format":"junit"}],
            "test_effectiveness":{"source_paths":["src/**"],"test_paths":["tests/**"],"assertion_failure_types":["AssertionError"]}})
    }
    #[test]
    fn strict_policy_and_reusable_plan_validate_effectiveness_without_yaml() {
        let good = command();
        assert!(validate(&serde_json::from_value(good.clone()).unwrap()).is_ok());
        for (pointer, bad) in [
            ("/kind", json!("manual")),
            ("/tools", json!([])),
            ("/findings_exit_codes", json!([])),
            ("/expected_exit_code", json!(1)),
            ("/reports", json!([])),
            ("/reports/0/format", json!("diagnostics")),
            ("/reports/0/mode", json!("changed_lines")),
            ("/reports/0/baseline", json!("old.xml")),
            ("/reports/0/minimum_coverage", json!(90)),
            ("/test_effectiveness/source_paths", json!([])),
            ("/test_effectiveness/test_paths", json!(["../escape"])),
            ("/test_effectiveness/test_paths", json!(["/absolute"])),
            ("/test_effectiveness/test_paths", json!(["tests//a"])),
            ("/test_effectiveness/test_paths", json!(["[broken"])),
            ("/test_effectiveness/test_paths", json!(["a", "a"])),
            ("/test_effectiveness/support_paths", json!(vec!["a"; 257])),
            ("/test_effectiveness/assertion_failure_types", json!([])),
            (
                "/test_effectiveness/assertion_failure_types",
                json!(["A", "A"]),
            ),
            ("/test_effectiveness/assertion_failure_types", json!([" "])),
            (
                "/test_effectiveness/assertion_failure_types",
                json!(["A\nB"]),
            ),
        ] {
            let mut bad_command = good.clone();
            if pointer == "/test_effectiveness/support_paths" {
                bad_command["test_effectiveness"]["support_paths"] = bad;
            } else if pointer.starts_with("/reports/0/") {
                bad_command["reports"][0][pointer.rsplit('/').next().unwrap()] = bad;
            } else if pointer == "/expected_exit_code" || pointer == "/kind" {
                bad_command[pointer.trim_start_matches('/')] = bad;
            } else {
                *bad_command.pointer_mut(pointer).unwrap() = bad;
            }
            let config: crate::config::Config =
                serde_json::from_value(json!({"schema_version":1,"checks":[bad_command]})).unwrap();
            assert!(
                crate::config::Plan::build(&config, None, "full").is_err(),
                "{pointer}"
            );
        }
        let mut unknown = good.clone();
        unknown["test_effectiveness"]["guess_runner"] = json!(true);
        assert!(serde_json::from_value::<CommandCheck>(unknown).is_err());
        let plain: CommandCheck =
            serde_json::from_value(json!({"id":"plain","argv":["git","--version"]})).unwrap();
        assert!(
            serde_json::to_value(plain)
                .unwrap()
                .get("test_effectiveness")
                .is_none()
        );
    }
}
