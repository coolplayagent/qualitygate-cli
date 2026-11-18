//! Live GCC C/C++ analyzer SARIF across two immutable Git snapshots.
mod common;
#[path = "common/repository.rs"]
mod repository;

use common::{cli, fixture, git, report};
use serde_json::{Value, json};
use std::{fs, path::Path, process::Command};

const C_FIRST: &str = "#include <stdlib.h>\nint first(int bad) {\n  int *value = malloc(sizeof *value);\n  if (bad) return 1;\n  free(value);\n  return 0;\n}\n";
const C_SECOND: &str = "int second(int bad) {\n  int *value = malloc(sizeof *value);\n  if (bad) return 1;\n  free(value);\n  return 0;\n}\n";
const C_DEPRECATION: &str = include_str!("../skills/qualitygate-cli/references/c-deprecation.yaml");
const CPP_DEPRECATION: &str =
    include_str!("../skills/qualitygate-cli/references/cpp-deprecation.yaml");

// These cases measure historical repository debt; delivery intersection is tested separately.
fn run(root: &Path, expected: i32) -> Value {
    let output = repository::check(root, &[]);
    report(&output, expected)
}

#[test]
fn gcc_reference_policy_is_valid_and_sarif_baseline_is_required() {
    let repository = fixture();
    let root = repository.path();
    let source = include_str!("../skills/qualitygate-cli/references/gcc-analyzer-ratchet.yaml");
    fs::write(root.join("qualitygate.yaml"), source).unwrap();
    assert_eq!(
        cli(root, &["config", "--show", "--format", "json"])
            .status
            .code(),
        Some(0)
    );
    fs::write(
        root.join("qualitygate.yaml"),
        source.replace("format: sarif", "format: unknown_c_analyzer"),
    )
    .unwrap();
    assert_eq!(
        cli(root, &["config", "--show", "--format", "json"])
            .status
            .code(),
        Some(2)
    );
    fs::write(
        root.join("qualitygate.yaml"),
        format!(
            "{}\n",
            source
                .lines()
                .filter(|line| !line.trim_start().starts_with("baseline:"))
                .collect::<Vec<_>>()
                .join("\n")
        ),
    )
    .unwrap();
    assert_eq!(
        cli(root, &["config", "--show", "--format", "json"])
            .status
            .code(),
        Some(2)
    );
}

#[test]
#[ignore = "requires GCC 13+ C and C++ SARIF producers; mandatory live C/C++ CI job"]
fn real_gcc_c_and_cpp_sarif_ratchets_growth_repair_and_compile_failure() {
    for (tool, extension, flags, historical, added, repaired, broken, rule_id) in [
        (
            "gcc", "c", Vec::<&str>::new(), C_FIRST.to_owned(), C_SECOND.to_owned(),
            "#include <stdlib.h>\nint first(int bad) { int *value = malloc(sizeof *value); free(value); return bad; }\n".to_owned(),
            "int first( {\n".to_owned(), "-Wanalyzer-malloc-leak",
        ),
        (
            "g++", "cpp", vec!["-Wall"], "int first() { int unused = 1; return 0; }\n".to_owned(),
            "int second() { int unused = 2; return 0; }\n".to_owned(),
            "int first() { return 0; }\n".to_owned(),
            "int first( {\n".to_owned(), "-Wunused-variable",
        ),
    ] {
        assert!(Command::new(tool).arg("--version").output().unwrap().status.success());
        let repository = fixture();
        let root = repository.path();
        let source_name = format!("sample.{extension}");
        let report_name = format!("{source_name}.sarif");
        fs::write(root.join(&source_name), &historical).unwrap();
        let mut argv = vec![tool.to_owned(), "-fanalyzer".into(), "-fdiagnostics-format=sarif-file".into()];
        argv.extend(flags.into_iter().map(str::to_owned));
        argv.extend(["-c".into(), source_name.clone(), "-o".into(), "sample.o".into()]);
        let policy = json!({"schema_version":1,"checks":[{
            "id":"c-family-ratchet", "argv":argv, "timeout_seconds":120,
            "tools":[{"id":"compiler","argv":[tool,"--version"]}],
            "reports":[{"path":report_name,"baseline":report_name,"format":"sarif","mode":"ratchet"}]
        }]});
        fs::write(root.join("qualitygate.yaml"), serde_norway::to_string(&policy).unwrap()).unwrap();
        git(root, &["add", "."]);
        git(root, &["commit", "-qm", "historical C family diagnostic"]);
        let initial = run(root, 0);
        assert_eq!(initial["gate"]["decision"], "pass", "{tool}: {initial}");
        assert_eq!(initial["checks"][0]["metadata"][format!("{report_name}:ratchet")][0]["baseline"], 1);

        fs::write(root.join(&source_name), format!("{historical}{added}")).unwrap();
        let growth = run(root, 1);
        let diagnostics = growth["checks"][0]["diagnostics"].as_array().unwrap();
        assert_eq!(diagnostics.len(), 2, "{tool}: {growth}");
        assert_eq!(diagnostics[0]["evidence"]["rule"], rule_id);
        assert_eq!(diagnostics[0]["file"], source_name);

        fs::write(root.join(&source_name), &repaired).unwrap();
        assert_eq!(run(root, 0)["gate"]["decision"], "pass");
        fs::write(root.join(&source_name), &broken).unwrap();
        let incomplete = run(root, 2);
        assert_eq!(incomplete["gate"]["decision"], "incomplete", "{tool}: {incomplete}");
    }
}

#[test]
fn packaged_c_family_deprecation_policies_are_valid() {
    let repository = fixture();
    let root = repository.path();
    for policy in [C_DEPRECATION, CPP_DEPRECATION] {
        fs::write(root.join("qualitygate.yaml"), policy).unwrap();
        report(&cli(root, &["config", "--show", "--format", "json"]), 0);
    }
}

#[test]
#[ignore = "requires GCC 13+ C and C++ SARIF producers; mandatory live C/C++ CI job"]
fn real_gcc_deprecation_checks_block_unchanged_c_and_cpp_callers() {
    for (policy, source_name, source, clean, deprecated) in [
        (
            C_DEPRECATION,
            "sample.c",
            "#include \"api.h\"\nint use_api(void) { return legacy(); }\n",
            "static inline int legacy(void) { return 0; }\n",
            "__attribute__((deprecated)) static inline int legacy(void) { return 0; }\n",
        ),
        (
            CPP_DEPRECATION,
            "sample.cpp",
            "#include \"api.h\"\nLegacy object;\n",
            "struct Legacy {};\n",
            "struct [[deprecated]] Legacy {};\n",
        ),
    ] {
        let repository = fixture();
        let root = repository.path();
        let header = root.join("api.h");
        fs::write(root.join("qualitygate.yaml"), policy).unwrap();
        fs::write(root.join(source_name), source).unwrap();
        fs::write(&header, clean).unwrap();
        git(root, &["add", "."]);
        git(
            root,
            &["commit", "-qm", "non-deprecated C family declaration"],
        );
        let check = |status| {
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
        };
        assert_eq!(check(0)["gate"]["decision"], "pass");

        fs::write(&header, deprecated).unwrap();
        let blocked = check(1);
        assert_eq!(blocked["selection"]["changed_files"], json!(["api.h"]));
        assert_eq!(blocked["gate"]["complete"], true);
        let result = &blocked["checks"][0];
        assert_eq!(result["execution"]["exit_code"], 0);
        let diagnostics = result["diagnostics"].as_array().unwrap();
        assert_eq!(diagnostics.len(), 1, "{source_name}: {diagnostics:?}");
        let finding = &diagnostics[0];
        assert_eq!(finding["file"], source_name);
        assert_eq!(finding["range"]["start_line"], 2);
        assert_eq!(finding["evidence"]["rule"], "-Wdeprecated-declarations");

        fs::write(&header, "struct broken {\n").unwrap();
        assert_eq!(check(2)["gate"]["decision"], "incomplete");
        fs::write(&header, clean).unwrap();
        assert_eq!(check(0)["gate"]["decision"], "pass");
    }
}
