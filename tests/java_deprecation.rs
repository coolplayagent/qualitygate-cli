mod common;

use common::{cli, fixture, git, report};
use serde_json::Value;
use std::{fs, path::Path, process::Command};

const JAVA_8_POLICY: &str =
    include_str!("../skills/qualitygate-cli/references/java-deprecation-jdk8.yaml");
const MODERN_POLICY: &str =
    include_str!("../skills/qualitygate-cli/references/java-deprecation.yaml");

const CONSUMER: &str = "package app;
public class Consumer implements deps.LegacyInterface {
    @deps.LegacyNote public void run() {
        new deps.LegacyType();
        new deps.Stable().old();
    }
}
";

fn checked_tool(root: &Path, tool: &str, args: &[&str]) {
    let output = Command::new(tool)
        .args(args)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{tool}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn current_javac_is_java_8() -> bool {
    let output = Command::new("javac").arg("-version").output().unwrap();
    assert!(output.status.success(), "{output:?}");
    let version = [output.stdout, output.stderr].concat();
    String::from_utf8(version)
        .unwrap()
        .split_whitespace()
        .any(|token| token.starts_with("1.8."))
}

fn dependency(root: &Path, deprecated: bool, removal_supported: bool) {
    let fixture = tempfile::tempdir().unwrap();
    let root_sources = fixture.path();
    let mark = if deprecated { "@Deprecated " } else { "" };
    let interface_mark = if deprecated && removal_supported {
        "@Deprecated(forRemoval = true) "
    } else {
        mark
    };
    for (name, declaration) in [
        ("LegacyType", format!("{mark}public class LegacyType {{}}")),
        (
            "LegacyInterface",
            format!("{interface_mark}public interface LegacyInterface {{}}"),
        ),
        (
            "LegacyNote",
            format!("{mark}public @interface LegacyNote {{}}"),
        ),
        (
            "Stable",
            format!("public class Stable {{ {mark}public void old() {{}} }}"),
        ),
    ] {
        fs::write(
            root_sources.join(format!("{name}.java")),
            format!("package deps; {declaration}\n"),
        )
        .unwrap();
    }
    fs::create_dir(root_sources.join("classes")).unwrap();
    checked_tool(
        root_sources,
        "javac",
        &[
            "-d",
            "classes",
            "LegacyType.java",
            "LegacyInterface.java",
            "LegacyNote.java",
            "Stable.java",
        ],
    );
    let jar = root.join("deps/api.jar");
    checked_tool(
        root_sources,
        "jar",
        &["cf", jar.to_str().unwrap(), "-C", "classes", "."],
    );
}

fn run(root: &Path, status: i32) -> Value {
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
        status,
    )
}

#[test]
fn packaged_java_deprecation_policy_is_valid() {
    let fixture = fixture();
    let root = fixture.path();
    let capabilities = report(&cli(root, &["capabilities", "--format", "json"]), 0);
    let advertised = capabilities["capabilities"].as_array().unwrap();
    for capability in ["reports.project-diagnostics.v1", "reports.javac.v1"] {
        assert!(advertised.iter().any(|item| item == capability));
    }
    for policy in [JAVA_8_POLICY, MODERN_POLICY] {
        fs::write(root.join("qualitygate.yaml"), policy).unwrap();
        assert_eq!(cli(root, &["config", "--show"]).status.code(), Some(0));
    }
}

#[test]
#[ignore = "requires JDK 8, 17, or 21; mandatory live Java guard CI job"]
fn dependency_only_upgrade_blocks_unchanged_java_callers_and_recovers() {
    let fixture = fixture();
    let root = fixture.path();
    for directory in ["src path/app", "src path/other", "config", "deps"] {
        fs::create_dir_all(root.join(directory)).unwrap();
    }
    let removal_supported = !current_javac_is_java_8();
    let policy = if removal_supported {
        MODERN_POLICY
    } else {
        JAVA_8_POLICY
    };
    fs::write(root.join("qualitygate.yaml"), policy).unwrap();
    fs::write(
        root.join("config/java-sources.args"),
        "-classpath deps/api.jar\n\"src path/app/Consumer.java\"\n\"src path/other/Consumer.java\"\n",
    )
    .unwrap();
    let primary = root.join("src path/app/Consumer.java");
    fs::write(&primary, CONSUMER).unwrap();
    fs::write(
        root.join("src path/other/Consumer.java"),
        "package other; public class Consumer { deps.LegacyType value; }\n",
    )
    .unwrap();
    dependency(root, false, removal_supported);
    git(root, &["add", "."]);
    git(
        root,
        &["commit", "-qm", "clean callers and original dependency"],
    );
    fs::write(root.join("hello.txt"), "delivery\n").unwrap();
    assert_eq!(run(root, 0)["gate"]["decision"], "pass");
    fs::write(root.join("hello.txt"), "initial\n").unwrap();

    dependency(root, true, removal_supported);
    let blocked = run(root, 1);
    assert_eq!(blocked["gate"]["complete"], true);
    assert_eq!(
        blocked["selection"]["changed_files"],
        serde_json::json!(["deps/api.jar"])
    );
    let check = &blocked["checks"][0];
    let diagnostics = check["diagnostics"].as_array().unwrap();
    for name in ["LegacyType", "LegacyInterface", "LegacyNote", "old()"] {
        assert!(
            diagnostics.iter().any(|item| {
                item["file"] == "src path/app/Consumer.java"
                    && item["message"].as_str().unwrap().contains(name)
            }),
            "missing {name} in {diagnostics:?}"
        );
    }
    assert!(diagnostics.iter().any(|item| {
        item["file"] == "src path/other/Consumer.java"
            && item["message"].as_str().unwrap().contains("LegacyType")
            && item["evidence"]["rule"] == "deprecation"
    }));
    let interface_rule = if removal_supported {
        "removal"
    } else {
        "deprecation"
    };
    assert!(diagnostics.iter().any(|item| {
        item["message"]
            .as_str()
            .unwrap()
            .contains("LegacyInterface")
            && item["evidence"]["rule"] == interface_rule
    }));

    fs::write(&primary, "package app; public class Consumer {\n").unwrap();
    assert_eq!(run(root, 2)["gate"]["decision"], "incomplete");
    fs::write(&primary, CONSUMER).unwrap();
    dependency(root, false, removal_supported);
    fs::write(root.join("hello.txt"), "repaired delivery\n").unwrap();
    assert_eq!(run(root, 0)["gate"]["decision"], "pass");
}
