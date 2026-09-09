use super::*;
use crate::snapshot::{File, Identity};
use serde_json::json;

const PREFIX: &str = "[INFO] --- clean:3.2.0:clean (default-clean) @ sample ---\n[INFO] --- compiler:3.13.0:testCompile (default-testCompile) @ sample ---\n[INFO] Compiling 1 source file with javac [debug target 17] to target/test-classes\n[INFO] --- dependency:3.8.1:analyze-only (default-cli) @ sample ---\n";
const SUFFIX: &str = "[INFO] ------------------------------------------------------------------------\n[INFO] BUILD SUCCESS\n";
const MISSING: &str = "[WARNING] Used undeclared dependencies found:\n[WARNING]    org.hamcrest:hamcrest-core:jar:1.3:test\n[WARNING] Unused declared dependencies found:\n[WARNING]    junit:junit:jar:4.13.2:test\n";

fn fixture() -> (Snapshot, ProjectFacts) {
    let facts: ProjectFacts = serde_json::from_value(json!({"schema_version":1,"ecosystem":"maven","root":".","manifest":"pom.xml","coordinate":"fixture:sample:1","source_root":"src/main/java","test_source_root":"src/test/java","producer_check":"facts","snapshot_digest":"digest",
        "declared":[{"group":"junit","artifact":"junit","artifact_type":"jar","classifier":"","version":"4.13.2","scope":"test"}],
        "resolved":[{"group":"junit","artifact":"junit","artifact_type":"jar","classifier":"","version":"4.13.2","scope":"test"},{"group":"org.hamcrest","artifact":"hamcrest-core","artifact_type":"jar","classifier":"","version":"1.3","scope":"test"}]})).unwrap();
    let snapshot = Snapshot {
        root: ".".into(),
        identity: Identity {
            mode: "worktree".into(),
            base: "base".into(),
            head: "head".into(),
            content_digest: "digest".into(),
            merge_request: None,
        },
        files: [
            (
                "src/test/java/T.java".into(),
                File {
                    bytes: b"class T {}".to_vec(),
                    executable: false,
                },
            ),
            (
                "pom.xml".into(),
                File {
                    bytes: b"<project/>".to_vec(),
                    executable: false,
                },
            ),
        ]
        .into(),
        base_files: Default::default(),
        changes: Default::default(),
        path_filter: None,
        commits: vec![],
    };
    (snapshot, facts)
}

#[test]
fn bytecode_usage_is_cross_checked_and_drives_repairable_diagnostics() {
    let (snapshot, mut facts) = fixture();
    let log = format!("{PREFIX}{MISSING}{SUFFIX}");
    let usage = parse(b"<project/>", log.as_bytes(), &facts, &snapshot).unwrap();
    assert_eq!(usage.compiled_test_sources, 1);
    assert_eq!(usage.used_undeclared[0].artifact, "hamcrest-core");
    facts.dependency_usage = Some(usage);
    let setting = crate::config::RuleSetting {
        parameters: serde_json::from_value(json!({"modules":["."]})).unwrap(),
        ..Default::default()
    };
    let run = |projects: &[ProjectFacts]| {
        crate::adapters::rules::evaluate_with_projects(
            "used-undeclared",
            "used-undeclared",
            &setting,
            &snapshot,
            projects,
        )
    };
    let failed = run(&[facts.clone()]);
    assert_eq!(failed.verdict, Some(crate::domain::Verdict::Fail));
    assert_eq!(failed.diagnostics[0].file.as_deref(), Some("pom.xml"));
    assert_eq!(failed.metadata["increment_mode"], "full");
    facts.declared.push(facts.resolved[1].clone());
    facts.dependency_usage = Some(
        parse(
            b"<project/>",
            format!("{PREFIX}[INFO] No dependency problems found\n{SUFFIX}").as_bytes(),
            &facts,
            &snapshot,
        )
        .unwrap(),
    );
    assert_eq!(
        run(&[facts.clone()]).verdict,
        Some(crate::domain::Verdict::Pass)
    );
    assert_eq!(
        run(&[]).execution.status,
        crate::domain::ExecutionStatus::Blocked
    );
    assert_eq!(
        run(&[facts.clone(), facts.clone()]).execution.status,
        crate::domain::ExecutionStatus::Blocked
    );
    facts.snapshot_digest = "foreign".into();
    assert_eq!(
        run(&[facts.clone()]).execution.status,
        crate::domain::ExecutionStatus::Blocked
    );
    facts.snapshot_digest = "digest".into();
    facts.dependency_usage = None;
    assert_eq!(
        run(&[facts]).execution.status,
        crate::domain::ExecutionStatus::Blocked
    );
}

#[test]
fn incomplete_filtered_foreign_and_contradictory_analysis_cannot_pass() {
    let (snapshot, facts) = fixture();
    let log = format!("{PREFIX}{MISSING}{SUFFIX}");
    for altered in [
        String::new(),
        log.replace("BUILD SUCCESS", "BUILD FAILURE"),
        log.replace("default-clean", "other-clean"),
        log.replace("1 source file", "2 source files"),
        log.replace("Compiling 1 source file", "Nothing to compile"),
        log.replace("3.8.1:analyze-only", "3.9.0:analyze-only"),
        log.replace("@ sample ---", "@ other ---"),
        format!("{PREFIX}[INFO] Skipping plugin execution\n{SUFFIX}"),
        format!("{PREFIX}[INFO] Skipping project with no build directory\n{SUFFIX}"),
        format!("{PREFIX}[WARNING] Used undeclared dependencies found:\n{SUFFIX}"),
        format!("{PREFIX}{MISSING}"),
        format!("{PREFIX}{MISSING}[INFO] No dependency problems found\n{SUFFIX}"),
        format!("{PREFIX}{MISSING}{MISSING}{SUFFIX}"),
        format!("{PREFIX}[INFO] No dependency problems found\n{MISSING}{SUFFIX}"),
        log.replace(
            "org.hamcrest:hamcrest-core:jar:1.3:test",
            "unknown:artifact:jar:1.3:test",
        ),
        log.replace(
            "org.hamcrest:hamcrest-core:jar:1.3:test",
            "junit:junit:jar:4.13.2:test",
        ),
        log.replace(
            "junit:junit:jar:4.13.2:test",
            "org.hamcrest:hamcrest-core:jar:1.3:test",
        ),
        log.replace(
            "org.hamcrest:hamcrest-core:jar:1.3:test",
            "org.hamcrest:hamcrest-core:jar:1.3:import",
        ),
        log.replace("org.hamcrest:hamcrest-core:jar:1.3:test", "invalid"),
        format!("{log}[INFO] --- compiler:3.13.0:compile (default-compile) @ sample ---\n"),
        format!("{PREFIX}Unable to process class\n{SUFFIX}"),
        format!("{PREFIX}[INFO] unexpected analyzer output\n{SUFFIX}"),
        format!("{log}\0"),
        format!("\x1b[0m{log}"),
    ] {
        assert!(
            parse(b"<project/>", altered.as_bytes(), &facts, &snapshot).is_err(),
            "{altered}"
        );
    }
    let mut empty = snapshot.clone();
    empty.files.clear();
    assert!(parse(b"<project/>", log.as_bytes(), &facts, &empty).is_err());
    assert!(parse(b"<project/>", &[0xff], &facts, &snapshot).is_err());
}

#[test]
fn suppression_overrides_and_custom_analyzers_require_explicit_support() {
    let (snapshot, facts) = fixture();
    let log = format!("{PREFIX}{MISSING}{SUFFIX}");
    let model = |artifact: &str, config: &str| {
        format!(
            "<project><build><plugins><plugin><artifactId>{artifact}</artifactId>{config}</plugin></plugins></build></project>"
        )
    };
    for config in [
        "<configuration><ignoredUsedUndeclaredDependencies><ignored>*</ignored></ignoredUsedUndeclaredDependencies></configuration>",
        "<configuration><excludedClasses>.*</excludedClasses></configuration>",
        "<configuration><analyzer>custom</analyzer></configuration>",
        "<dependencies/>",
    ] {
        assert!(
            parse(
                model("maven-dependency-plugin", config).as_bytes(),
                log.as_bytes(),
                &facts,
                &snapshot
            )
            .is_err()
        );
    }
    for key in [
        "skip",
        "excludes",
        "includes",
        "compilerId",
        "compilerArgs",
        "outputDirectory",
    ] {
        assert!(
            parse(
                model(
                    "maven-compiler-plugin",
                    &format!("<configuration><{key}>custom</{key}></configuration>")
                )
                .as_bytes(),
                log.as_bytes(),
                &facts,
                &snapshot
            )
            .is_err()
        );
    }
    assert!(parse(model("maven-dependency-plugin","<configuration><verbose>false</verbose><analyzer>default</analyzer></configuration>").as_bytes(),log.as_bytes(),&facts,&snapshot).is_ok());
    assert_eq!(
        coordinate("g:a:test-jar:tests:1:test").unwrap().classifier,
        "tests"
    );
}

#[test]
fn main_and_test_compilations_have_separate_complete_inventories() {
    let (mut snapshot, facts) = fixture();
    snapshot.files.insert(
        "src/main/java/Main.java".into(),
        File {
            bytes: b"class Main {}".to_vec(),
            executable: false,
        },
    );
    let prefix = PREFIX.replace("[INFO] --- compiler:3.13.0:testCompile", "[INFO] --- compiler:3.13.0:compile (default-compile) @ sample ---\n[INFO] Compiling 1 source file with javac to target/classes\n[INFO] --- compiler:3.13.0:testCompile");
    let log = format!("{prefix}{MISSING}{SUFFIX}");
    let usage = parse(b"<project/>", log.as_bytes(), &facts, &snapshot).unwrap();
    assert_eq!(usage.compiled_main_sources, 1);
    assert_eq!(usage.compiled_test_sources, 1);
    let skipped = log.replace(
        "Compiling 1 source file with javac to target/classes",
        "Nothing to compile",
    );
    assert!(parse(b"<project/>", skipped.as_bytes(), &facts, &snapshot).is_err());
    let wrong = log.replace("(default-compile)", "(other-module)");
    assert!(parse(b"<project/>", wrong.as_bytes(), &facts, &snapshot).is_err());
    let cleared = log.replace(
        "[INFO] --- dependency:",
        "[INFO] --- clean:3.2.0:clean (default-clean) @ sample ---\n[INFO] --- dependency:",
    );
    assert!(parse(b"<project/>", cleared.as_bytes(), &facts, &snapshot).is_err());
}
