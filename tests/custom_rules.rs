mod common;
use common::*;
use std::path::Path;

const SOURCE: &str =
    "# Team rules\nRequire descriptive test names and honest source declarations.\n";

fn policy(root: &Path, tail: &str) {
    std::fs::create_dir_all(root.join("private-rules")).unwrap();
    std::fs::write(root.join("AGENTS.md"), SOURCE).unwrap();
    std::fs::write(
        root.join("qualitygate.yaml"),
        format!(
            "schema_version: 1\ncustom_rules: private-rules\nrules: {{private-rule: {{}}}}\n{tail}"
        ),
    )
    .unwrap();
}

fn definition(root: &Path, body: &str) {
    std::fs::write(root.join("private-rules/private.yaml"), format!(
        "id: private-rule\nversion: 1\nsource:\n  document: AGENTS.md\n  section: Team rules\n  content_hash: {}\nfix: Follow the team convention and rerun the check\n{body}\n",
        qualitygate::snapshot::digest(SOURCE.as_bytes()),
    )).unwrap();
}

fn check(root: &Path, code: i32) -> serde_json::Value {
    report(&cli(root, &["check", "--format", "json"]), code)
}

const NAMING: &str = "language: [java, python]\nrequires_capabilities: [test_methods]\nwhen: {entity: test_method, change: added}\nthen: {name_pattern: '^test_descriptive_.+$'}";

#[test]
fn private_rule_runs_on_java_and_python_with_stable_diagnostics_and_snapshot_isolation() {
    let root = fixture();
    let root = root.path();
    policy(root, "");
    definition(root, NAMING);
    std::fs::write(root.join("T.java"), "class T { @Test void wrong() {} }\n").unwrap();
    std::fs::write(root.join("test_t.py"), "def test_wrong():\n    pass\n").unwrap();
    let failed = check(root, 1);
    assert_eq!(failed["checks"][0]["matched_entities"], 2);
    assert_eq!(
        failed["checks"][0]["diagnostics"].as_array().unwrap().len(),
        2
    );
    assert_eq!(
        failed["checks"][0]["metadata"]["rule_definition"]["origin"],
        "private-rules/private.yaml"
    );
    git(root, &["add", "."]);
    std::fs::write(root.join("test_t.py"), "\n\ndef test_wrong():\n    pass\n").unwrap();
    let moved = check(root, 1);
    assert_eq!(
        failed["checks"][0]["diagnostics"][1]["fingerprint"],
        moved["checks"][0]["diagnostics"][1]["fingerprint"]
    );
    std::fs::write(
        root.join("test_t.py"),
        "def test_descriptive_python():\n    pass\n",
    )
    .unwrap();
    std::fs::write(
        root.join("T.java"),
        "class T { @Test void test_descriptive_java() {} }\n",
    )
    .unwrap();
    check(root, 0);
    let staged = report(&cli(root, &["check", "--staged", "--format", "json"]), 1);
    assert_eq!(staged["checks"][0]["matched_entities"], 2);
}

#[test]
fn definitions_defaults_discovery_enable_and_formats_share_effective_policy() {
    let root = fixture();
    let root = root.path();
    policy(root, "");
    definition(root, &format!("severity: warning\n{NAMING}"));
    std::fs::write(root.join("test_t.py"), "def test_wrong():\n    pass\n").unwrap();
    let checked = check(root, 0);
    assert_eq!(checked["checks"][0]["severity"], "warning");
    assert_eq!(
        checked["checks"][0]["diagnostics"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let config = report(&cli(root, &["config", "--show", "--format", "json"]), 0);
    assert_eq!(
        config["config"]["rules"]["private-rule"]["severity"],
        "warning"
    );
    let listed = report(&cli(root, &["rules", "list", "--format", "json"]), 0);
    assert!(
        listed["rules"]
            .as_array()
            .unwrap()
            .iter()
            .any(|rule| rule["id"] == "private-rule" && rule["enabled"] == true)
    );
    report(
        &cli(
            root,
            &["rules", "enable", "parameterized-tests", "--format", "json"],
        ),
        0,
    );
    let shown = report(&cli(root, &["config", "--show", "--format", "json"]), 0);
    assert_eq!(
        shown["config"]["rules"]["parameterized-tests"]["severity"],
        "warning"
    );
    for format in ["table", "markdown"] {
        let list = cli(root, &["rules", "list", "--format", format]);
        assert!(list.status.success());
        let text = String::from_utf8(list.stdout).unwrap();
        assert!(text.contains("private-rule") && text.contains("test_methods"));
        let config = cli(root, &["config", "--show", "--format", format]);
        assert!(config.status.success());
        assert!(
            String::from_utf8(config.stdout)
                .unwrap()
                .contains("private-rules/private.yaml")
        );
    }
    report(
        &cli(root, &["rules", "enable", "missing", "--format", "json"]),
        2,
    );
}

#[test]
fn changed_source_or_definition_invalidates_policy() {
    let root = fixture();
    let root = root.path();
    policy(root, "");
    definition(root, NAMING);
    std::fs::write(
        root.join("test_t.py"),
        "def test_descriptive_python():\n    pass\n",
    )
    .unwrap();
    let initial = check(root, 0);
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "trusted policy"]);
    definition(root, &NAMING.replace("test_descriptive_", "test_"));
    let changed = check(root, 0);
    assert_ne!(
        initial["policy"]["rules_digest"],
        changed["policy"]["rules_digest"]
    );
    let blocked = report(
        &cli(root, &["check", "--policy-ref", "HEAD", "--format", "json"]),
        2,
    );
    assert!(
        blocked["policy"]["changes"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("private-rules/private.yaml"))
    );
    std::fs::write(
        root.join("AGENTS.md"),
        format!("{SOURCE}Changed requirement.\n"),
    )
    .unwrap();
    let changed = check(root, 2);
    assert!(
        changed["checks"][0]["execution"]["reason"]
            .as_str()
            .unwrap()
            .contains("digest changed")
    );
}

#[test]
fn custom_defaults_are_resolved_before_full_profile_completeness() {
    let root = fixture();
    let root = root.path();
    policy(root, "");
    definition(root, &format!("required: false\n{NAMING}"));
    std::fs::write(root.join("qualitygate.yaml"), "schema_version: 1\ncustom_rules: private-rules\nrules: {private-rule: {}, line-ending: {}}\nprofiles: {full: {include: [line-ending]}}\n").unwrap();
    check(root, 0);
    definition(root, NAMING);
    check(root, 2);
}

#[test]
fn rule_revision_and_protocol_version_are_independent() {
    let root = fixture();
    let root = root.path();
    policy(root, "");
    definition(root, NAMING);
    let initial = check(root, 0);
    let path = root.join("private-rules/private.yaml");
    let revised = std::fs::read_to_string(&path)
        .unwrap()
        .replace("version: 1", "version: 2");
    std::fs::write(&path, format!("schema_version: 1\n{revised}")).unwrap();
    let updated = check(root, 0);
    assert_eq!(updated["checks"][0]["rule_version"], 2);
    assert_ne!(
        updated["policy"]["rules_digest"],
        initial["policy"]["rules_digest"]
    );
}

#[test]
fn replacing_builtin_definition_is_explicit_and_fully_reported() {
    let root = fixture();
    let root = root.path();
    policy(root, "");
    definition(root, NAMING);
    let path = root.join("private-rules/private.yaml");
    std::fs::write(
        &path,
        std::fs::read_to_string(&path)
            .unwrap()
            .replace("id: private-rule", "id: line-ending"),
    )
    .unwrap();
    std::fs::write(
        root.join("qualitygate.yaml"),
        "schema_version: 1\ncustom_rules: private-rules\nrules: {line-ending: {}}\n",
    )
    .unwrap();
    std::fs::write(root.join("test_x.py"), "def test_wrong():\n    pass\n").unwrap();
    let result = check(root, 1);
    assert_eq!(result["checks"][0]["id"], "line-ending");
    assert_eq!(
        result["checks"][0]["metadata"]["rule_definition"]["package"],
        "custom"
    );
    assert!(
        result["checks"][0]["diagnostics"][0]["message"]
            .as_str()
            .unwrap()
            .contains("name pattern")
    );
}

#[test]
fn text_assertions_examine_ast_spans_even_when_two_tests_share_a_line() {
    let root = fixture();
    let root = root.path();
    policy(root, "");
    definition(
        root,
        "language: [java]\nrequires_capabilities: [test_methods]\nwhen: {entity: test_method, change: added}\nthen: {forbid_pattern: forbidden}",
    );
    std::fs::write(
        root.join("T.java"),
        "class T { @Test void bad() { forbidden(); } @Test void good() { allowed(); } }",
    )
    .unwrap();
    let result = check(root, 1);
    assert_eq!(result["checks"][0]["matched_entities"], 2);
    assert_eq!(
        result["checks"][0]["diagnostics"].as_array().unwrap().len(),
        1
    );
    assert!(
        result["checks"][0]["diagnostics"][0]["evidence"]["symbol"]
            .as_str()
            .unwrap()
            .contains("bad")
    );
}

#[test]
fn invalid_dsl_definitions_never_silently_reduce_checks() {
    let root = fixture();
    let root = root.path();
    policy(root, "");
    for body in [
        "requires_capabilities: [test_methods]\nwhen: {entity: test_method}\nthen: {}",
        "requires_capabilities: [test_methods]\nwhen: {entity: test_method}\nthen: {name_patern: wrong}",
        "requires_capabilities: []\nwhen: {entity: test_method}\nthen: {name_pattern: ok}",
        "requires_capabilities: [unknown]\nwhen: {entity: test_method}\nthen: {name_pattern: ok}",
        "requires_capabilities: [test_methods]\nwhen: {entity: test_method}\nthen: {name_pattern: '['}",
        "requires_capabilities: [test_methods]\nwhen: {entity: test_method, change: deleted}\nthen: {name_pattern: ok}",
        "requires_capabilities: [test_methods]\nwhen: {entity: test_method}\nthen: {require_marker: true}",
    ] {
        definition(root, body);
        check(root, 2);
    }
    definition(root, NAMING);
    std::fs::copy(
        root.join("private-rules/private.yaml"),
        root.join("private-rules/duplicate.yaml"),
    )
    .unwrap();
    check(root, 2);
    std::fs::remove_file(root.join("private-rules/duplicate.yaml")).unwrap();
    let path = root.join("private-rules/private.yaml");
    let yaml = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, format!("schema_version: 2\n{yaml}")).unwrap();
    check(root, 2);
    std::fs::write(&path, yaml.replace("version: 1", "version: 0")).unwrap();
    check(root, 2);
    std::fs::remove_file(path).unwrap();
    check(root, 2);
}

#[test]
fn capability_gaps_and_ai_only_scope_are_incomplete_even_without_matching_entities() {
    let root = fixture();
    let root = root.path();
    policy(root, "");
    for body in [
        "language: [ruby]\nrequires_capabilities: [test_methods]\nwhen: {entity: test_method}\nthen: {name_pattern: ok}",
        "language: [java]\nrequires_capabilities: [test_methods, dependency_resolution]\nwhen: {entity: test_method}\nthen: {require_dependency: {group: example, artifact: declarations}}",
        "language: [python]\nrequires_capabilities: [test_methods, comments]\napplies_to: {provenance_scope: ai_only}\nbinding: {marker: {type: comment, name: '@generated'}}\nwhen: {entity: test_method}\nthen: {require_marker: true}",
    ] {
        definition(root, body);
        let blocked = check(root, 2);
        assert_eq!(blocked["checks"][0]["verdict"], serde_json::Value::Null);
    }
}

#[test]
fn marker_fields_need_real_assignments_and_comments_must_bind_to_the_entity() {
    let root = fixture();
    let root = root.path();
    policy(root, "");
    definition(
        root,
        "language: [java]\nrequires_capabilities: [test_methods, annotations]\napplies_to: {provenance_scope: all_added_tests}\nbinding: {marker: {type: annotation, name: Generated, fields: [author]}}\nwhen: {entity: test_method}\nthen: {require_marker: true}",
    );
    for source in [
        "class T { @Test @Generated(author=\"\") void test() {} }",
        "class T { @Test @Generated(description=\"author='someone'\") void test() {} }",
    ] {
        std::fs::write(root.join("T.java"), source).unwrap();
        check(root, 1);
    }
    std::fs::write(
        root.join("T.java"),
        "class T { @Test @Generated(author=\"someone\") void test() {} }",
    )
    .unwrap();
    check(root, 0);
    definition(
        root,
        "language: [python]\nrequires_capabilities: [test_methods, comments]\napplies_to: {provenance_scope: all_added_tests}\nbinding: {marker: {type: comment, name: '@generated', fields: [author]}}\nwhen: {entity: test_method}\nthen: {require_marker: true}",
    );
    for source in [
        "# @generated author='me'\nx = 1\ndef test_x():\n    pass\n",
        "# unrelated @generated author='me'\ndef test_x():\n    pass\n",
    ] {
        std::fs::write(root.join("test_x.py"), source).unwrap();
        check(root, 1);
    }
    std::fs::write(
        root.join("test_x.py"),
        "# @generated author='me'\n@pytest.mark.slow\ndef test_x():\n    pass\n",
    )
    .unwrap();
    check(root, 0);
}

#[test]
fn comments_files_commits_and_max_count_use_their_declared_scope() {
    let root = fixture();
    let root = root.path();
    policy(root, "");
    definition(
        root,
        "language: [python]\nrequires_capabilities: [comments]\nwhen: {entity: comment, change: added}\nthen: {forbid_pattern: TODO, max_count: 1}",
    );
    std::fs::write(root.join("test_x.py"), "# TODO fix\n# explanation\nx = 1\n").unwrap();
    let failed = check(root, 1);
    assert_eq!(
        failed["checks"][0]["diagnostics"].as_array().unwrap().len(),
        2
    );
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "historic"]);
    std::fs::write(
        root.join("test_x.py"),
        "\n# TODO fix\n# explanation\nx = 2\n",
    )
    .unwrap();
    assert_eq!(check(root, 0)["checks"][0]["matched_entities"], 0);
    definition(
        root,
        "requires_capabilities: [files]\napplies_to: {paths: ['*.txt']}\nwhen: {entity: file, change: modified}\nthen: {forbid_pattern: secret}",
    );
    std::fs::write(root.join("hello.txt"), "secret\n").unwrap();
    check(root, 1);
    definition(
        root,
        "requires_capabilities: [commits]\nwhen: {entity: commit, change: added}\nthen: {name_pattern: '^\\[ID\\]fix: .+'}",
    );
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "bad subject"]);
    let failed = report(
        &cli(
            root,
            &["check", "--diff", "HEAD~1..HEAD", "--format", "json"],
        ),
        1,
    );
    assert_eq!(failed["checks"][0]["matched_entities"], 1);
}

#[test]
fn selected_init_path_and_language_package_activation_are_explicit() {
    let root = fixture();
    let root = root.path();
    report(
        &cli(
            root,
            &["init", "--config", "alternate.yaml", "--format", "json"],
        ),
        0,
    );
    assert!(!root.join("qualitygate.yaml").exists());
    let original = std::fs::read(root.join("alternate.yaml")).unwrap();
    report(
        &cli(
            root,
            &["init", "--config", "alternate.yaml", "--format", "json"],
        ),
        0,
    );
    assert_eq!(
        std::fs::read(root.join("alternate.yaml")).unwrap(),
        original
    );
    std::fs::write(root.join("qualitygate.yaml"), "schema_version: 1\nrulesets: [lang-java, lang-python]\nrules: {junit-naming: {}, pytest-naming: {}}\n").unwrap();
    std::fs::write(root.join("T.java"), "class T { @Test void bad() {} }").unwrap();
    std::fs::write(root.join("test_x.py"), "def test_correct():\n    pass\n").unwrap();
    let failed = check(root, 1);
    assert_eq!(failed["checks"][0]["id"], "junit-naming");
    assert!(
        failed["checks"][0]["diagnostics"][0]["id"]
            .as_str()
            .unwrap()
            .starts_with("junit-naming:")
    );
    assert_eq!(failed["checks"][1]["verdict"], "pass");
}

#[test]
fn moves_copies_and_overloads_are_compared_as_entities() {
    let root = fixture();
    let root = root.path();
    policy(root, "");
    definition(root, NAMING);
    std::fs::write(
        root.join("test_x.py"),
        "def test_old():\n    assert 1 == 1\n",
    )
    .unwrap();
    std::fs::write(
        root.join("T.java"),
        "class T { @Test void test_old() {} }\n",
    )
    .unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "legacy tests"]);
    std::fs::remove_file(root.join("test_x.py")).unwrap();
    std::fs::write(
        root.join("test_y.py"),
        "def test_renamed():\n    assert 1 == 1\n\ndef test_copied():\n    assert 1 == 1\n",
    )
    .unwrap();
    std::fs::write(
        root.join("T.java"),
        "class T { @Test void test_old() {} @Test void test_old(int value) {} }\n",
    )
    .unwrap();
    let changed = check(root, 1);
    assert_eq!(changed["checks"][0]["matched_entities"], 2);
    let messages = changed["checks"][0]["diagnostics"].to_string();
    assert!(messages.contains("test_copied") && messages.contains("test_old(int)"));
    assert!(!messages.contains("test_renamed"));
    definition(root, &NAMING.replace("change: added", "change: renamed"));
    assert_eq!(check(root, 1)["checks"][0]["matched_entities"], 1);
    definition(root, &NAMING.replace("change: added", "change: any"));
    assert_eq!(check(root, 1)["checks"][0]["matched_entities"], 4);
}
