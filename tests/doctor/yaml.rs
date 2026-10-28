use super::*;

#[test]
fn expanded_yaml_is_incomplete_before_config_or_task_execution_and_recovers() {
    let repo = fixture();
    let root = repo.path();
    let config = root.join("qualitygate.yaml");
    let task = root.join("task.yaml");
    let valid = b"schema_version: 1\nchecks: [{id: version, argv: [&git git, --version], tools: [{id: git, argv: [*git, --version]}]}]\n";
    let cases = [
        (
            format!(
                "large: &a '{}'\nextra: [{}]\n",
                "x".repeat(131_072),
                vec!["*a"; 32].join(",")
            ),
            "strings exceed 4194304 bytes",
        ),
        (
            format!("extra: [{}]\n", vec!["0"; 100_000].join(",")),
            "exceeds 100000 nodes",
        ),
        (
            format!("extra: {}null{}\n", "[".repeat(64), "]".repeat(64)),
            "nesting exceeds 64 levels",
        ),
        (
            format!(
                "large: &a -0x{}1\nextra: [{}]\n",
                "0".repeat(131_072),
                vec!["*a"; 128].join(",")
            ),
            "scalar text exceeds 16777216 bytes",
        ),
    ];
    for (body, reason) in cases {
        let invalid = format!("schema_version: 1\n{body}");
        std::fs::write(&config, &invalid).unwrap();
        for args in [
            vec!["config", "--show", "--format", "json"],
            vec!["check", "--worktree", "--format", "json"],
        ] {
            let result = report(&cli(root, &args), 2);
            assert!(has_code(&result, "config.syntax"), "{result}");
            assert!(
                result["diagnostics"].to_string().contains(reason),
                "{result}"
            );
            assert_eq!(result["gate"]["complete"], false);
        }
        let result = doctor(root, &[], 2);
        assert!(has_code(&result, "config.syntax"));
        assert!(result["diagnostics"].to_string().contains(reason));
        assert_eq!(std::fs::read(&config).unwrap(), invalid.as_bytes());

        std::fs::write(&config, valid).unwrap();
        std::fs::write(&task, &invalid).unwrap();
        let result = report(
            &cli(
                root,
                &[
                    "check",
                    "--worktree",
                    "--task",
                    "task.yaml",
                    "--format",
                    "json",
                ],
            ),
            2,
        );
        assert!(has_code(&result, "config.syntax"), "{result}");
        assert!(result["diagnostics"].to_string().contains(reason));
        assert_eq!(result["gate"]["complete"], false);
        assert_eq!(std::fs::read(&task).unwrap(), invalid.as_bytes());
        assert_eq!(std::fs::read(&config).unwrap(), valid);
    }
    std::fs::remove_file(task).unwrap();
    doctor(root, &[], 0);
    let result = report(
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
        0,
    );
    assert_eq!(result["gate"]["complete"], true);
    assert_eq!(result["gate"]["decision"], "pass");
}
