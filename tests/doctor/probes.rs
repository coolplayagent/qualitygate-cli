use super::*;

#[test]
fn failed_timed_out_and_overflow_probes_retain_incomplete_evidence() {
    let root = fixture();
    let tool = producer();
    let binary = tool.path().join("probe.exe");
    for (mode, code) in [
        ("failure", "probe.exit_code"),
        ("timeout-long", "probe.timeout"),
        ("overflow", "probe.output_limit"),
    ] {
        let mut value = simple();
        value["checks"][0]["tools"] =
            json!([{"id":"tool","argv":[binary,mode],"timeout_seconds":1}]);
        policy(root.path(), value);
        let result = doctor(root.path(), &["--probe-tools"], 2);
        assert!(has_code(&result, code), "{result}");
        assert_eq!(result["checks"][0]["probes"][0]["attempted"], true);
        assert_eq!(result["checks"][0]["probes"][0]["complete"], false);
        assert!(result["checks"][0]["probes"][0]["evidence"].is_object());
    }
}

#[cfg(unix)]
fn script(root: &Path, contents: &str) {
    use std::os::unix::fs::PermissionsExt;
    let path = root.join("probe.sh");
    std::fs::write(&path, contents).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    let mut value = simple();
    value["checks"][0]["tools"] =
        json!([{"id":"script","argv":["./probe.sh"],"inputs":["probe.sh"],"timeout_seconds":1}]);
    policy(root, value);
}

#[cfg(unix)]
#[test]
fn probes_use_snapshot_scripts_and_never_fall_back_to_worktree_bytes() {
    let root = fixture();
    script(root.path(), "#!/bin/sh\nprintf 'committed-version\\n'\n");
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-qm", "snapshot tool"]);
    script(root.path(), "#!/bin/sh\nexit 7\n");
    for args in [
        vec!["--staged", "--probe-tools"],
        vec!["--diff", "HEAD~1..HEAD", "--probe-tools"],
    ] {
        let result = doctor(root.path(), &args, 0);
        assert_eq!(
            result["checks"][0]["probes"][0]["evidence"]["version_digest"],
            qualitygate::snapshot::digest(b"committed-version")
        );
    }
    assert!(has_code(
        &doctor(root.path(), &["--probe-tools"], 2),
        "probe.exit_code"
    ));
}

#[cfg(unix)]
#[test]
fn empty_versions_input_tampering_and_replaced_tools_are_incomplete() {
    let root = fixture();
    script(root.path(), "#!/bin/sh\nexit 0\n");
    assert!(has_code(
        &doctor(root.path(), &["--probe-tools"], 2),
        "probe.version_invalid"
    ));
    script(
        root.path(),
        "#!/bin/sh\nprintf 'changed\\n' > hello.txt\nprintf '1.0\\n'\n",
    );
    assert!(has_code(
        &doctor(root.path(), &["--probe-tools"], 2),
        "snapshot.inputs_changed"
    ));
    assert_eq!(
        std::fs::read_to_string(root.path().join("hello.txt")).unwrap(),
        "initial\n"
    );
    script(
        root.path(),
        "#!/bin/sh\nprintf '#!/bin/sh\\necho changed\\n' > replacement\nchmod +x replacement\nmv replacement probe.sh\nprintf '1.0\\n'\n",
    );
    let result = doctor(root.path(), &["--probe-tools"], 2);
    assert!(has_code(&result, "probe.executable_changed"));
    assert!(has_code(&result, "snapshot.inputs_changed"));
}

#[cfg(unix)]
#[test]
fn policy_changes_during_probes_and_probe_stdout_secrets_are_not_accepted() {
    let root = fixture();
    script(
        root.path(),
        "#!/bin/sh\nprintf '%s\\n' \"$QUALITYGATE_SECRET_39\"\n",
    );
    let output = Command::new(env!("CARGO_BIN_EXE_qualitygate"))
        .env("QUALITYGATE_SECRET_39", "sensitive-output-39")
        .args([
            "--root",
            root.path().to_str().unwrap(),
            "doctor",
            "--probe-tools",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    report(&output, 0);
    assert!(!String::from_utf8_lossy(&output.stdout).contains("sensitive-output-39"));
    let path = root
        .path()
        .join("qualitygate.yaml")
        .to_str()
        .unwrap()
        .replace('\'', "'\\''");
    script(
        root.path(),
        &format!("#!/bin/sh\nprintf '\\n# changed\\n' >> '{path}'\nprintf '1.0\\n'\n"),
    );
    let value = doctor(root.path(), &["--probe-tools"], 2);
    assert!(has_code(&value, "snapshot.source_changed"));
    assert!(has_code(&value, "policy.changed"));
}
