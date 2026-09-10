use super::*;
use std::path::Path;

fn write(root: &Path, path: &str, text: &str) {
    let file = root.join(path);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(file, text).unwrap();
}

#[test]
fn nested_languages_manifests_ignore_rules_and_capability_gaps_are_explicit() {
    let root = tempfile::tempdir().unwrap();
    write(root.path(), ".gitignore", "ignored/\n*.rb\n!keep.rb\n");
    write(root.path(), "ignored/pom.xml", "<project/>");
    write(root.path(), "node_modules/package.json", "{}");
    write(root.path(), "target/Cargo.toml", "[package]");
    write(root.path(), "java/pom.xml", "<project/>");
    write(root.path(), "java/src/Test.java", "class Test {}");
    write(
        root.path(),
        "py/pyproject.toml",
        "[project]\nname='demo'\nversion='1.0'\n[project.optional-dependencies]\ntest=['pytest>=8']\n",
    );
    write(root.path(), "py/requirements.txt", "pytest==8.4.2\n");
    write(root.path(), "py/test_api.py", "def test_api(): pass\n");
    write(root.path(), "tools/check.sh", "exit 0\n");
    write(root.path(), "ignored.rb", "");
    write(root.path(), "keep.rb", "");
    write(root.path(), "nested/.gitignore", "!deep.rb\n");
    write(root.path(), "nested/deep.rb", "");
    let report = discover(root.path()).unwrap();
    assert!(!report.commands_executed);
    assert_eq!(
        report
            .languages
            .iter()
            .map(|value| value.language.as_str())
            .collect::<Vec<_>>(),
        ["java", "python", "ruby", "shell"]
    );
    assert_eq!(
        report
            .languages
            .iter()
            .find(|value| value.language == "ruby")
            .unwrap()
            .source_files,
        2
    );
    assert_eq!(report.projects.len(), 3);
    assert!(
        report
            .projects
            .iter()
            .any(|value| value.root == "py"
                && value.project_capabilities == ["dependency_resolution"])
    );
    assert!(report.available_rulesets.contains_key("lang-java"));
    assert!(report.available_rulesets.contains_key("lang-python"));
    let shell = report
        .languages
        .iter()
        .find(|value| value.language == "shell")
        .unwrap();
    assert_eq!(shell.syntax_capabilities, ["comments"]);
    assert!(shell.marker_options.is_empty());
    assert!(
        report
            .gaps
            .iter()
            .any(|value| value.reason == "No syntax adapter for ruby")
    );
    assert!(report.inputs.contains_key("nested/.gitignore"));
    assert_eq!(
        report
            .suggested_checks
            .iter()
            .filter(|value| value.purpose == "pytest")
            .count(),
        1
    );
    for suggestion in &report.suggested_checks {
        let config = Config {
            checks: vec![suggestion.check.clone()],
            ..Config::default()
        };
        crate::config::parse(serde_norway::to_string(&config).unwrap().as_bytes()).unwrap();
    }
}

#[test]
fn command_suggestions_preserve_declared_tools_and_report_requirements() {
    let root = tempfile::tempdir().unwrap();
    write(root.path(), "rust/Cargo.toml", "[workspace]\nmembers=[]\n");
    write(root.path(), "rust/Cargo.lock", "version = 4");
    write(root.path(), "java/pom.xml", "<project/>");
    write(root.path(), "java/settings.xml", "<settings/>");
    write(root.path(), "java/mvnw", "must not execute");
    write(root.path(), "java/mvnw.cmd", "must not execute");
    write(
        root.path(),
        "web/package.json",
        r#"{"packageManager":"pnpm@10.0.0","scripts":{"build":"some-build","test":"some-test","lint":"some-lint"}}"#,
    );
    write(root.path(), "web/pnpm-lock.yaml", "");
    write(
        root.path(),
        "go/go.mod",
        "module example.test/app\n\ngo 1.24\n",
    );
    let report = discover(root.path()).unwrap();
    let cargo = report
        .suggested_checks
        .iter()
        .find(|value| value.purpose == "cargo-test")
        .unwrap();
    assert_eq!(
        cargo.check.argv,
        ["cargo", "test", "--locked", "--workspace"]
    );
    let maven = report
        .suggested_checks
        .iter()
        .find(|value| value.purpose == "maven-verify")
        .unwrap();
    assert_eq!(maven.status, "report_mapping_required");
    assert!(maven.check.argv[0].starts_with("./mvnw"));
    assert_eq!(maven.check.required_args, ["-s", "settings.xml"]);
    let node = report
        .suggested_checks
        .iter()
        .find(|value| value.purpose == "pnpm-test")
        .unwrap();
    assert_eq!(node.check.argv, ["pnpm", "run", "test"]);
    assert_eq!(node.status, "report_mapping_required");
    assert_eq!(node.source, "web/package.json#scripts.test");
    assert!(
        report
            .suggested_checks
            .iter()
            .any(|value| value.purpose == "go-test" && value.status == "report_mapping_required")
    );
}

#[test]
fn malformed_unsupported_and_dynamic_projects_do_not_gain_semantic_capabilities() {
    let root = tempfile::tempdir().unwrap();
    for (file, text) in [
        ("broken/Cargo.toml", "not TOML"),
        ("xml/pom.xml", "<wrong/>"),
        ("json/package.json", "[]"),
        ("manager/package.json", r#"{"packageManager":"yarn@4"}"#),
        (
            "py/pyproject.toml",
            "[project]\nname='demo'\ndynamic=['version']",
        ),
        ("legacy/setup.py", "raise Exception('must not execute')"),
        ("gradle/build.gradle", "plugins {}"),
        ("go/go.mod", "not a module"),
    ] {
        write(root.path(), file, text);
    }
    write(root.path(), "manager/package-lock.json", "{}");
    let report = discover(root.path()).unwrap();
    assert!(
        report
            .projects
            .iter()
            .all(|value| value.project_capabilities.is_empty())
    );
    assert_eq!(
        report
            .projects
            .iter()
            .find(|value| value.ecosystem == "gradle")
            .unwrap()
            .metadata_status,
        "detected_only"
    );
    assert!(
        report
            .gaps
            .iter()
            .any(|value| value.reason.contains("Conflicting package manager"))
    );
    assert!(report.suggested_checks.is_empty());
    for file in [
        "setup.py",
        "setup.cfg",
        "Gemfile",
        "composer.json",
        "App.csproj",
    ] {
        assert!(inventory::manifest_kind(file).is_some());
    }
}

#[test]
fn budgets_and_invalid_ignore_inputs_fail_before_candidate_publication() {
    let root = tempfile::tempdir().unwrap();
    write(root.path(), "Cargo.toml", &"x".repeat(2 * 1024 * 1024 + 1));
    assert!(
        crate::config::initialize_at(root.path(), Path::new("qualitygate.yaml"), true).is_err()
    );
    assert!(!root.path().join("qualitygate.yaml").exists());
    write(root.path(), "Cargo.toml", "[package]");
    write(root.path(), ".gitignore", "[z-a]");
    assert!(discover(root.path()).is_err());
    std::fs::remove_file(root.path().join(".gitignore")).unwrap();
    let deep = "d/".repeat(65);
    write(
        root.path(),
        &format!("{deep}go.mod"),
        "module example.test/app",
    );
    assert!(discover(root.path()).is_err());
}

#[cfg(unix)]
#[test]
fn discovery_never_follows_symlink_manifests_or_ignore_files() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    write(outside.path(), "secret", "SECRET");
    for file in ["pom.xml", ".gitignore"] {
        symlink(outside.path().join("secret"), root.path().join(file)).unwrap();
        assert!(discover(root.path()).is_err());
        std::fs::remove_file(root.path().join(file)).unwrap();
    }
}
