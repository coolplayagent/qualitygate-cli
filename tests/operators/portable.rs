//! The compiled DSL must not acquire a dependency on a language runtime.
use super::*;
use std::{io::ErrorKind, os::unix::fs::symlink, time::Duration};

async fn isolated_cli(root: &Path, tools: &Path, args: &[&str]) -> std::process::Output {
    let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_qualitygate"));
    command
        .kill_on_drop(true)
        .env_clear()
        .env("PATH", tools)
        .env("TMPDIR", tools)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env(
            "QUALITYGATE_HOME",
            root.join(".git/qualitygate-test-evidence"),
        )
        .env(
            "QUALITYGATE_BUILTIN_RULES_DIR",
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/skills/qualitygate-cli/references/rules"
            ),
        )
        .current_dir(root)
        .arg("--root")
        .arg(root)
        .args(args);
    // Keep instrumented CLI coverage outside the fixture's captured inputs.
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        command.env("LLVM_PROFILE_FILE", profile);
    }
    tokio::time::timeout(Duration::from_secs(60), command.output())
        .await
        .expect("runtime-independent CLI exceeded 60 seconds")
        .unwrap()
}

#[tokio::test]
async fn python_and_typescript_lambdas_need_only_the_cli_and_git() {
    let tools = tempfile::tempdir().unwrap();
    symlink(which::which("git").unwrap(), tools.path().join("git")).unwrap();
    for executable in [
        "cargo", "rustc", "rustup", "node", "nodejs", "python", "python3",
    ] {
        let error = std::process::Command::new(executable)
            .env_clear()
            .env("PATH", tools.path())
            .current_dir(tools.path())
            .output()
            .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::NotFound, "{executable}: {error}");
    }

    for (language, extension, content) in [
        ("python", "py", "def run():\n    return True\n"),
        (
            "typescript",
            "ts",
            "export function run() { return true; }\n",
        ),
    ] {
        let dir = setup();
        let root = dir.path();
        let expected = json!({"op":"map","input":{"op":"entities"},"as":"source",
            "body":{"op":"replace","value":{"op":"field","name":"path","of":{"op":"var","name":"source"}},"from":"src/","to":"tests/"}});
        let actual = json!({"op":"map","input":{"op":"source","name":"tests"},"as":"test","body":{"op":"field","name":"path"}});
        let mut candidate = rule(json!({"op":"subset","left":expected,"right":actual}));
        candidate["language"] = json!([language]);
        candidate["applies_to"]["paths"] = json!([format!("src/**/*.{extension}")]);
        candidate["sources"]["tests"]["language"] = json!([language]);
        candidate["sources"]["tests"]["applies_to"]["paths"] =
            json!([format!("tests/**/*.{extension}")]);
        write_candidate(root, &candidate);
        report(
            &isolated_cli(
                root,
                tools.path(),
                &["rules", "validate", "candidate.yaml", "--format", "json"],
            )
            .await,
            0,
        );
        install(root, &candidate);
        std::fs::create_dir(root.join("tests")).unwrap();
        let counterpart = root.join(format!("tests/behavior.{extension}"));
        std::fs::write(&counterpart, content).unwrap();
        git(root, &["add", "."]);
        git(root, &["commit", "-qm", "existing counterpart"]);
        std::fs::write(root.join(format!("src/behavior.{extension}")), content).unwrap();

        let args = [
            "check",
            "--worktree",
            "--profile",
            "full",
            "--format",
            "json",
        ];
        let passed = report(&isolated_cli(root, tools.path(), &args).await, 0);
        assert_eq!(passed["gate"]["complete"], true);
        assert_eq!(
            passed["checks"][0]["metadata"]["expression"]["sources"],
            json!({"entities":1,"tests":1})
        );

        std::fs::remove_file(&counterpart).unwrap();
        let failed = report(&isolated_cli(root, tools.path(), &args).await, 1);
        assert_eq!(failed["gate"]["complete"], true);
        assert_eq!(
            failed["checks"][0]["metadata"]["expression"]["passed"],
            false
        );
        assert_ne!(
            passed["snapshot"]["content_digest"],
            failed["snapshot"]["content_digest"]
        );

        std::fs::write(&counterpart, content).unwrap();
        std::fs::write(root.join("AGENTS.md"), "# Tests\nChanged requirement.\n").unwrap();
        let stale = report(&isolated_cli(root, tools.path(), &args).await, 2);
        assert_eq!(stale["gate"]["complete"], false);
        assert!(
            stale["checks"][0]["execution"]["reason"]
                .as_str()
                .unwrap()
                .contains("digest changed")
        );
    }
}
