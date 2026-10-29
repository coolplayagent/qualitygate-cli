use super::*;

fn saved(summary: &Value) -> (Value, std::path::PathBuf) {
    qualitygate::config::preflight_schema::validate_doctor_summary(summary).unwrap();
    let path = std::path::PathBuf::from(summary["report"]["path"].as_str().unwrap());
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(summary["report"]["bytes"], bytes.len());
    assert_eq!(
        summary["report"]["digest"],
        qualitygate::snapshot::digest(&bytes)
    );
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    qualitygate::config::preflight_schema::validate_doctor(&value).unwrap();
    assert_eq!(value["complete"], summary["complete"]);
    assert_eq!(
        value["diagnostics"].as_array().unwrap().len(),
        summary["counts"]["diagnostics"]
    );
    (value, path)
}

#[test]
fn summary_keeps_large_blocked_report_in_unique_temporary_directories() {
    let root = fixture();
    let checks: Vec<_> = (0..60)
        .map(|i| {
            json!({
                "id": format!("check-{i}"), "argv": ["git", "--version"],
                "cwd": format!("missing-directory-{i}"),
            })
        })
        .collect();
    policy(root.path(), json!({"schema_version": 1, "checks": checks}));
    let mut paths = Vec::new();
    for _ in 0..2 {
        let output = cli(root.path(), &["doctor", "--summary", "--format", "json"]);
        let summary = report(&output, 2);
        assert_eq!(summary["kind"], "doctor_summary");
        assert_eq!(summary["scope"], "preflight");
        assert_eq!(summary["exit_code"], 2);
        assert_eq!(summary["details_truncated"], true);
        assert_eq!(summary["counts"]["diagnostics"], 60);
        assert_eq!(summary["diagnostics"].as_array().unwrap().len(), 5);
        assert!(output.stdout.len() < 5000);
        let (full, path) = saved(&summary);
        assert!(
            full["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| { d["checks"].as_array().unwrap().contains(&json!("check-59")) })
        );
        assert!(
            full["next_steps"]
                .as_array()
                .unwrap()
                .iter()
                .any(|s| s["command"].is_array())
        );
        assert!(
            std::fs::canonicalize(&path)
                .unwrap()
                .starts_with(std::fs::canonicalize(std::env::temp_dir()).unwrap())
        );
        assert!(!path.starts_with(root.path()));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(path.parent().unwrap())
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o077,
                0
            );
        }
        paths.push(path);
    }
    assert_ne!(paths[0], paths[1]);
    for path in paths {
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
    assert_eq!(git_status(root.path()), "?? qualitygate.yaml\n");
}

fn git_status(root: &Path) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["status", "--porcelain"])
        .output()
        .unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn human_output_defaults_to_summary_and_full_json_remains_compatible() {
    let root = fixture();
    policy(root.path(), simple());
    for format in ["table", "markdown"] {
        let output = cli(root.path(), &["doctor", "--format", format]);
        assert!(output.status.success());
        let text = String::from_utf8(output.stdout).unwrap();
        let text = text.trim();
        let text = text.strip_prefix("```yaml\n").unwrap_or(text);
        let text = text.strip_suffix("```").unwrap_or(text);
        let summary: Value = serde_norway::from_str(text).unwrap();
        assert_eq!(summary["exit_code"], 0);
        assert_eq!(summary["details_truncated"], false);
        let (full, path) = saved(&summary);
        assert!(!full["not_executed"].as_array().unwrap().is_empty());
        assert!(summary.get("gate").is_none());
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
    let full = doctor(root.path(), &[], 0);
    assert!(full.get("kind").is_none());
    assert!(full.get("report").is_none());
}

#[test]
fn malformed_policy_and_invalid_selection_keep_diagnostics_in_saved_report() {
    let root = fixture();
    std::fs::write(
        root.path().join("qualitygate.yaml"),
        "schema_version: 1\nchecks: [\n",
    )
    .unwrap();
    for args in [
        vec!["doctor", "--summary", "--format", "json"],
        vec!["doctor", "--summary", "--format", "json", "--diff", "bad"],
    ] {
        let summary = report(&cli(root.path(), &args), 2);
        let (full, path) = saved(&summary);
        assert!(!full["diagnostics"].as_array().unwrap().is_empty());
        assert_eq!(summary["exit_code"], 2);
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}

#[test]
fn unavailable_temporary_storage_is_incomplete_without_a_fake_report_path() {
    let root = fixture();
    policy(root.path(), simple());
    let unavailable = root.path().join("hello.txt");
    let output = Command::new(env!("CARGO_BIN_EXE_qualitygate"))
        .arg("--root")
        .arg(root.path())
        .args(["doctor", "--summary", "--format", "json"])
        .env("TMPDIR", &unavailable)
        .env("TMP", &unavailable)
        .env("TEMP", &unavailable)
        .env(
            "QUALITYGATE_BUILTIN_RULES_DIR",
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/skills/qualitygate-cli/references/rules"
            ),
        )
        .output()
        .unwrap();
    let summary = report(&output, 2);
    qualitygate::config::preflight_schema::validate_doctor_summary(&summary).unwrap();
    assert_eq!(summary["complete"], false);
    assert_eq!(summary["exit_code"], 2);
    assert_eq!(summary["diagnostics"][0]["code"], "report.write_failed");
    assert!(summary["report"].is_null());
}
