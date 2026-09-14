mod common;
#[path = "common/reviews.rs"]
mod reviews;
use common::*;
use serde_json::{Value, json};
use std::path::Path;

fn run(root: &Path, args: &[&str]) -> Value {
    let mut args = args.to_vec();
    args.extend(["--format", "json"]);
    report(&cli(root, &args), 0)
}

fn policy(root: &Path) -> Value {
    serde_norway::from_slice(&std::fs::read(root.join("qualitygate.yaml")).unwrap()).unwrap()
}

fn rejected(root: &Path, args: &[&str]) {
    let before = std::fs::read(root.join("qualitygate.yaml")).unwrap();
    let mut args = args.to_vec();
    args.extend(["--format", "json"]);
    let output = cli(root, &args);
    assert_eq!(
        output.status.code(),
        Some(2),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    if !output.stdout.is_empty() {
        report(&output, 2);
    } else {
        assert!(!output.stderr.is_empty());
    }
    assert_eq!(
        std::fs::read(root.join("qualitygate.yaml")).unwrap(),
        before
    );
    assert!(!root.join("qualitygate.yaml.lock").exists());
}

#[test]
fn progressive_discovery_categories_are_mutable_and_do_not_enable_rules() {
    let directory = fixture();
    let root = directory.path();
    let categories = run(root, &["rules", "categories"]);
    assert!(
        categories["categories"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["name"] == "core"
                && c["rule_count"] == 3
                && c["enabled_count"] == 0
                && c["custom"] == false)
    );
    assert!(!root.join("qualitygate.yaml").exists());
    run(root, &["init"]);
    let before = policy(root);
    run(
        root,
        &[
            "rules",
            "categories",
            "create",
            "migration",
            "--description",
            "Rules for API migration",
        ],
    );
    run(
        root,
        &[
            "rules",
            "assign",
            "import-boundary",
            "--category",
            "migration",
        ],
    );
    run(
        root,
        &[
            "rules",
            "assign",
            "security-sensitive-api",
            "--category",
            "migration",
        ],
    );
    let filtered = run(
        root,
        &[
            "rules",
            "list",
            "--category",
            "migration",
            "--language",
            "rust",
            "--source",
            "builtin",
        ],
    );
    assert_eq!(filtered["rules"].as_array().unwrap().len(), 2);
    assert!(
        filtered["rules"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["enabled"] == false)
    );
    assert_eq!(policy(root)["rules"], before["rules"]);
    let description = run(root, &["rules", "describe", "security-sensitive-api"]);
    assert_eq!(description["category"], "migration");
    assert_eq!(description["implementation"], "source-pattern");
    assert_eq!(description["defaults"]["severity"], "warning");
    assert!(!description["standard_refs"].as_array().unwrap().is_empty());
    let param = description["parameters"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "prohibited_patterns")
        .unwrap();
    assert_eq!(param["type"], "object");
    assert!(param["default"]["java"].is_array());
    run(
        root,
        &[
            "rules",
            "categories",
            "rename",
            "migration",
            "api-migration",
        ],
    );
    assert_eq!(
        policy(root)["rule_categories"]["import-boundary"],
        "api-migration"
    );
    rejected(root, &["rules", "categories", "delete", "api-migration"]);
    run(
        root,
        &["rules", "categories", "delete", "api-migration", "--force"],
    );
    assert_eq!(
        run(root, &["rules", "describe", "import-boundary"])["category"],
        "architecture"
    );
    run(
        root,
        &["rules", "categories", "rename", "architecture", "layers"],
    );
    assert_eq!(
        run(root, &["rules", "describe", "import-boundary"])["category"],
        "layers"
    );
    let categories = run(root, &["rules", "categories"]);
    assert!(
        categories["categories"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["name"] == "layers" && c["custom"] == false)
    );
    run(root, &["rules", "categories", "delete", "layers"]);
    assert!(run(root, &["rules", "describe", "import-boundary"])["category"].is_null());
    run(root, &["rules", "categories", "create", "layers"]);
    assert!(run(root, &["rules", "describe", "import-boundary"])["category"].is_null());
    for format in ["table", "markdown"] {
        for args in [
            vec!["rules", "categories", "--format", format],
            vec!["rules", "describe", "test-naming", "--format", format],
            vec!["rules", "list", "--category", "test", "--format", format],
        ] {
            let output = cli(root, &args);
            assert!(output.status.success());
            assert!(!output.stdout.is_empty());
        }
    }
}

#[test]
fn configure_validates_types_merges_dotted_defaults_and_preserves_overrides() {
    let directory = fixture();
    let root = directory.path();
    std::fs::write(root.join("qualitygate.yaml"), "# retained on no-op\nschema_version: 1\nrules: {line-ending: {}, security-sensitive-api: {enabled: false}}\nprofiles: {full: {include: [line-ending]}, quick: {include: [line-ending]}}\n").unwrap();
    let before = run(root, &["rules", "describe", "security-sensitive-api"]);
    let mutation = run(
        root,
        &[
            "rules",
            "configure",
            "security-sensitive-api",
            "--param",
            r#"prohibited_patterns.java=["\\bRuntime\\.exec\\b"]"#,
        ],
    );
    assert_eq!(mutation["changed"], true);
    assert_ne!(mutation["before_digest"], mutation["after_digest"]);
    assert_eq!(
        policy(root)["rules"]["security-sensitive-api"]["parameters"]["prohibited_patterns"]["python"],
        before["defaults"]["parameters"]["prohibited_patterns"]["python"]
    );
    assert!(
        policy(root)["rules"]["line-ending"]
            .as_object()
            .unwrap()
            .is_empty()
    );
    assert!(policy(root)["rules"]["security-sensitive-api"]["severity"].is_null());
    run(root, &["rules", "enable", "security-sensitive-api"]);
    assert!(
        policy(root)["profiles"]["full"]["include"]
            .as_array()
            .unwrap()
            .contains(&json!("security-sensitive-api"))
    );
    assert_eq!(
        run(root, &["config", "--show"])["config"]["rules"]["security-sensitive-api"]["severity"],
        "warning"
    );
    run(root, &["rules", "disable", "security-sensitive-api"]);
    let bytes = std::fs::read(root.join("qualitygate.yaml")).unwrap();
    assert_eq!(
        run(root, &["rules", "disable", "security-sensitive-api"])["changed"],
        false
    );
    assert_eq!(std::fs::read(root.join("qualitygate.yaml")).unwrap(), bytes);
    run(
        root,
        &[
            "rules",
            "configure",
            "import-boundary",
            "--param",
            r#"forbidden_imports.rust=["^crate::interfaces"]"#,
            "--severity",
            "warning",
            "--required",
            "false",
        ],
    );
    assert_eq!(policy(root)["rules"]["import-boundary"]["enabled"], false);
    run(root, &["rules", "enable", "import-boundary"]);
    run(
        root,
        &[
            "rules",
            "configure",
            "junit-naming",
            "--param",
            r#"pattern="^test_""#,
        ],
    );
    assert!(
        policy(root)["rulesets"]
            .as_array()
            .unwrap()
            .contains(&json!("lang-java"))
    );
    run(root, &["rules", "enable", "junit-naming"]);
    assert_eq!(
        run(root, &["rules", "describe", "junit-naming"])["enabled"],
        true
    );
    for args in [
        vec!["rules", "configure", "test-naming", "--param", "unknown=1"],
        vec![
            "rules",
            "configure",
            "test-naming",
            "--param",
            "pattern=123",
        ],
        vec![
            "rules",
            "configure",
            "test-naming",
            "--param",
            r#"pattern="[""#,
        ],
        vec![
            "rules",
            "configure",
            "test-naming",
            "--param",
            "patterns.rust=123",
        ],
        vec![
            "rules",
            "configure",
            "test-naming",
            "--param",
            "patterns={}",
            "--param",
            r#"patterns.rust="foo""#,
        ],
        vec!["rules", "configure", "test-naming", "--param", "pattern"],
        vec![
            "rules",
            "configure",
            "test-naming",
            "--param",
            "pattern=not-json",
        ],
        vec![
            "rules",
            "configure",
            "test-naming",
            "--param",
            "pattern.child=1",
        ],
        vec![
            "rules",
            "configure",
            "test-naming",
            "--param",
            "patterns..rust=1",
        ],
        vec![
            "rules",
            "configure",
            "diff-size",
            "--param",
            "max_added_lines=0",
        ],
        vec![
            "rules",
            "configure",
            "security-sensitive-api",
            "--param",
            "prohibited_patterns.java=[]",
        ],
        vec!["rules", "configure", "test-naming"],
        vec!["rules", "configure", "test-naming", "--severity", "eror"],
        vec!["rules", "enable", "missing"],
        vec!["rules", "disable", "missing"],
    ] {
        rejected(root, &args);
    }
}

#[test]
fn category_errors_and_lock_conflicts_never_partially_publish() {
    let directory = fixture();
    let root = directory.path();
    run(root, &["init"]);
    for args in [
        vec!["rules", "categories", "create", "core"],
        vec!["rules", "categories", "create", "bad/name"],
        vec![
            "rules",
            "categories",
            "create",
            "name",
            "--description",
            "bad\ntext",
        ],
        vec!["rules", "categories", "rename", "core", "style"],
        vec!["rules", "categories", "rename", "missing", "new"],
        vec!["rules", "categories", "delete", "missing"],
        vec!["rules", "assign", "missing", "--category", "core"],
        vec!["rules", "assign", "line-ending", "--category", "missing"],
        vec!["rules", "list", "--category", "missing"],
        vec!["rules", "describe", "missing"],
    ] {
        rejected(root, &args);
    }
    let bytes = std::fs::read(root.join("qualitygate.yaml")).unwrap();
    std::fs::write(root.join("qualitygate.yaml.lock"), "held by another writer").unwrap();
    report(
        &cli(
            root,
            &["rules", "disable", "line-ending", "--format", "json"],
        ),
        2,
    );
    assert_eq!(std::fs::read(root.join("qualitygate.yaml")).unwrap(), bytes);
    assert_eq!(
        std::fs::read_to_string(root.join("qualitygate.yaml.lock")).unwrap(),
        "held by another writer"
    );
    std::fs::remove_file(root.join("qualitygate.yaml.lock")).unwrap();
    std::fs::create_dir(root.join("policies")).unwrap();
    std::fs::rename(
        root.join("qualitygate.yaml"),
        root.join("policies/team.yaml"),
    )
    .unwrap();
    run(
        root,
        &[
            "--config",
            "policies/team.yaml",
            "rules",
            "categories",
            "create",
            "migration",
        ],
    );
    assert!(!root.join("qualitygate.yaml").exists());
    assert!(
        run(
            root,
            &["--config", "policies/team.yaml", "rules", "categories"]
        )["categories"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["name"] == "migration")
    );
}

#[test]
fn cli_disabled_rule_stays_blocked_by_selected_trusted_policy() {
    let directory = fixture();
    let root = directory.path();
    run(root, &["init"]);
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "policy baseline"]);
    run(root, &["rules", "disable", "line-ending"]);
    std::fs::write(root.join("hello.txt"), "violation\r\n").unwrap();
    let checked = report(
        &cli(
            root,
            &[
                "check",
                "--worktree",
                "--profile",
                "quick",
                "--policy-ref",
                "HEAD",
                "--format",
                "json",
            ],
        ),
        2,
    );
    assert!(
        checked["checks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["id"] == "line-ending" && c["verdict"] == "fail")
    );
}

#[cfg(unix)]
#[test]
fn mutations_preserve_permissions_and_reject_symlink_policy() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let directory = fixture();
    let root = directory.path();
    run(root, &["init"]);
    let path = root.join("qualitygate.yaml");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640)).unwrap();
    run(root, &["rules", "disable", "line-ending"]);
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o640
    );
    std::fs::rename(&path, root.join("original.yaml")).unwrap();
    symlink("original.yaml", &path).unwrap();
    rejected(root, &["rules", "enable", "line-ending"]);
}

#[test]
fn every_builtin_parameter_contract_describes_and_validates_its_packaged_default() {
    let directory = fixture();
    let root = directory.path();
    let list = run(root, &["rules", "list", "--source", "builtin"]);
    for row in list["rules"].as_array().unwrap() {
        let id = row["id"].as_str().unwrap();
        let described = run(root, &["rules", "describe", id]);
        assert_eq!(
            described["version"],
            row["definition"]["builtin"]["version"]
        );
        for parameter in described["parameters"].as_array().unwrap() {
            assert!(!parameter["description"].as_str().unwrap().is_empty());
            let validator = jsonschema::draft202012::options()
                .build(&parameter["schema"])
                .unwrap();
            if parameter["has_default"] == true {
                assert!(
                    validator.is_valid(&parameter["default"]),
                    "{id}: {parameter}"
                );
            }
        }
    }
}

#[test]
fn project_discovery_and_assignment_work_before_activation_with_bounded_large_repo_cost() {
    let directory = fixture();
    let root = directory.path();
    std::fs::create_dir_all(root.join("qualitygate/rules")).unwrap();
    std::fs::create_dir(root.join("irrelevant")).unwrap();
    for index in 0..18_000 {
        std::fs::write(
            root.join(format!("irrelevant/{index}.txt")),
            b"unrelated repository content",
        )
        .unwrap();
    }
    let source = "# Rules\nReview dangerous patterns.\n";
    std::fs::write(root.join("AGENTS.md"), source).unwrap();
    for index in 0..256 {
        let id = format!("project-{index:03}");
        let definition = json!({"id":id,"version":1,"language":["rust"],
            "source":{"document":"AGENTS.md","section":"Rules","content_hash":qualitygate::snapshot::digest(source.as_bytes())},
            "requires_capabilities":["files"],"when":{"entity":"file"},"then":{"forbid_pattern":"danger"},"fix":"Review dangerous patterns."});
        std::fs::write(
            root.join(format!("qualitygate/rules/{id}.yaml")),
            serde_json::to_vec(&definition).unwrap(),
        )
        .unwrap();
    }
    // Rule queries read only policy and rule directories, never the source-tree inventory.
    for args in [
        vec!["rules", "categories"],
        vec!["rules", "list", "--category", "project"],
        vec!["rules", "describe", "project-007"],
    ] {
        let start = std::time::Instant::now();
        let result = run(root, &args);
        let elapsed = start.elapsed();
        eprintln!(
            "issue3 discovery: source_files=18000 project_rules=256 command={args:?} elapsed={elapsed:?}"
        );
        assert!(
            elapsed < std::time::Duration::from_secs(5),
            "Discovery exceeded 5 seconds: {elapsed:?}"
        );
        if args[1] == "categories" {
            assert!(
                result["categories"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|c| c["name"] == "project"
                        && c["rule_count"] == 256
                        && c["enabled_count"] == 0)
            );
            assert!(serde_json::to_vec(&result).unwrap().len() < 2048);
        } else if args[1] == "list" {
            assert_eq!(result["rules"].as_array().unwrap().len(), 256);
        } else {
            assert_eq!(result["implementation"], "project-dsl");
            assert!(result["parameters"].as_array().unwrap().is_empty());
        }
    }
    std::fs::write(
        root.join("qualitygate.yaml"),
        "schema_version: 1\nrules: {}\n",
    )
    .unwrap();
    run(root, &["rules", "categories", "create", "migration"]);
    run(
        root,
        &["rules", "assign", "project-007", "--category", "migration"],
    );
    assert!(policy(root)["custom_rules"].is_null());
    assert!(policy(root)["rules"].as_object().unwrap().is_empty());
    rejected(
        root,
        &["rules", "configure", "project-007", "--param", "pattern=1"],
    );
    run(root, &["rules", "enable", "project-007"]);
    assert_eq!(policy(root)["custom_rules"], "qualitygate/rules");
    assert_eq!(
        run(root, &["rules", "describe", "project-007"])["enabled"],
        true
    );
}

#[test]
fn category_edits_preserve_reviews_and_executable_edits_expose_stale_bindings() {
    let directory = fixture();
    let root = directory.path();
    let source = "# Rules\nRequire LF endings.\n";
    std::fs::write(root.join("AGENTS.md"), source).unwrap();
    let config = json!({"schema_version":1,"rules":{"line-ending":{"source":{
        "document":"AGENTS.md","section":"Rules","content_hash":qualitygate::snapshot::digest(source.as_bytes())}}}});
    std::fs::write(
        root.join("qualitygate.yaml"),
        serde_norway::to_string(&config).unwrap(),
    )
    .unwrap();
    reviews::record(root).unwrap();
    let initial = run(root, &["rules", "list"])["source_reviews"]["line-ending"].clone();
    run(root, &["rules", "categories", "create", "migration"]);
    run(
        root,
        &["rules", "assign", "line-ending", "--category", "migration"],
    );
    assert_eq!(
        run(root, &["rules", "list"])["source_reviews"]["line-ending"],
        initial
    );
    let record = policy(root)["source_reviews"]["line-ending"].clone();
    run(
        root,
        &["rules", "configure", "line-ending", "--severity", "warning"],
    );
    assert_eq!(policy(root)["source_reviews"]["line-ending"], record);
    let stale = run(root, &["rules", "list"]);
    assert_eq!(stale["source_reviews"]["line-ending"]["status"], "stale");
    report(
        &cli(
            root,
            &[
                "check",
                "--worktree",
                "--profile",
                "quick",
                "--format",
                "json",
            ],
        ),
        2,
    );
}

#[test]
fn configure_preserves_deliberately_scoped_profiles_and_validates_required_promotion() {
    let directory = fixture();
    let root = directory.path();
    std::fs::write(root.join("qualitygate.yaml"), "schema_version: 1\nrules: {test-naming: {required: false}}\nprofiles: {quick: {include: []}, full: {include: []}}\n").unwrap();
    let original = policy(root)["profiles"].clone();
    run(
        root,
        &[
            "rules",
            "configure",
            "test-naming",
            "--param",
            r#"patterns.rust="^test_""#,
        ],
    );
    assert_eq!(policy(root)["profiles"], original);
    rejected(
        root,
        &["rules", "configure", "test-naming", "--required", "true"],
    );
    run(root, &["rules", "enable", "test-naming"]);
    run(
        root,
        &["rules", "configure", "test-naming", "--required", "true"],
    );
    assert!(
        policy(root)["profiles"]["full"]["include"]
            .as_array()
            .unwrap()
            .contains(&json!("test-naming"))
    );
}
