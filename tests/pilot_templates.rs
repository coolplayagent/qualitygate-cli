mod common;
use common::*;
use std::process::Command;

#[test]
fn remaining_templates_execute_real_contracts_and_require_external_performance_review() {
    for name in ["feature", "dependency", "documentation", "performance"] {
        let repository = fixture();
        let root = repository.path();
        std::fs::create_dir(root.join("src")).unwrap();
        std::fs::create_dir(root.join("tests")).unwrap();
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]\nname='contract_fixture'\nversion='0.1.0'\nedition='2021'\n",
        )
        .unwrap();
        std::fs::write(
            root.join("src/lib.rs"),
            "pub fn supported() -> bool { false }\n",
        )
        .unwrap();
        std::fs::write(
            root.join(format!("tests/{name}_contract.rs")),
            "#[test] fn reviewed_contract() { assert!(contract_fixture::supported()); }\n",
        )
        .unwrap();
        std::fs::write(root.join(".gitignore"), "target/\n").unwrap();
        std::fs::write(root.join("qualitygate.yaml"), "schema_version: 1\n").unwrap();
        std::fs::copy(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(format!("templates/pilot/rust-v1/{name}.yaml")),
            root.join("task.yaml"),
        )
        .unwrap();
        assert!(
            Command::new("cargo")
                .args(["generate-lockfile", "--offline"])
                .current_dir(root)
                .output()
                .unwrap()
                .status
                .success()
        );
        git(root, &["add", "."]);
        git(root, &["commit", "-qm", "trusted contract"]);
        let check = |code| {
            report(
                &cli(
                    root,
                    &[
                        "check",
                        "--task",
                        "task.yaml",
                        "--policy-ref",
                        "HEAD",
                        "--format",
                        "json",
                    ],
                ),
                code,
            )
        };
        let before = check(if name == "performance" { 2 } else { 1 });
        let find = |value: &serde_json::Value| {
            value["checks"]
                .as_array()
                .unwrap()
                .iter()
                .find(|v| v["id"] == format!("rust-{name}"))
                .unwrap()
                .clone()
        };
        let result = find(&before);
        assert_eq!(result["verdict"], "fail");
        assert_eq!(result["execution"]["status"], "completed");
        assert_eq!(result["metadata"]["tests"]["executed"], 1);
        std::fs::write(
            root.join("src/lib.rs"),
            "pub fn supported() -> bool { true }\n",
        )
        .unwrap();
        let after = check(if name == "performance" { 2 } else { 0 });
        assert_eq!(find(&after)["verdict"], "pass");
        if name == "performance" {
            assert_eq!(after["gate"]["complete"], false);
            let manual = after["checks"]
                .as_array()
                .unwrap()
                .iter()
                .find(|v| v["id"] == "performance-review")
                .unwrap();
            assert!(manual["verdict"].is_null());
        }
        std::fs::write(
            root.join(format!("tests/{name}_contract.rs")),
            "// no scenarios\n",
        )
        .unwrap();
        let empty = check(if name == "performance" { 2 } else { 1 });
        assert_eq!(find(&empty)["metadata"]["tests"]["executed"], 0);
        assert_eq!(find(&empty)["verdict"], "fail");
    }
}
