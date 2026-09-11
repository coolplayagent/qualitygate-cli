mod common;
use common::*;
use serde_json::{Value, json};
use std::{path::Path, process::Command};

const SOURCE: &str = "# Team rules\nDeclare the source of changed tests.\n";
const PYTHON: &str = "def test_created():\n    assert True\n";
const MARKED: &str = "tests\n\nAI-Generated: author='agent' reason='test creation'";
const IDS: [&str; 2] = ["ai-code-traceability", "private-trailer"];

fn fixture_with_policy() -> tempfile::TempDir {
    let root = fixture();
    std::fs::create_dir_all(root.path().join("rules")).unwrap();
    std::fs::write(root.path().join("AGENTS.md"), SOURCE).unwrap();
    std::fs::write(root.path().join("rules/trailer.yaml"), format!(
        "id: private-trailer\nversion: 1\nsource: {{document: AGENTS.md, section: Team rules, content_hash: {}}}\nlanguage: [python, java]\nrequires_capabilities: [test_methods, commits]\napplies_to: {{provenance_scope: all_added_tests}}\nbinding: {{marker: {{type: git_trailer, name: AI-Generated, fields: [author, reason]}}}}\nwhen: {{entity: test_method, change: added}}\nthen: {{require_marker: true}}\nfix: Commit the actual test changes with the required declaration, then recheck\n",
        qualitygate::snapshot::digest(SOURCE.as_bytes()))).unwrap();
    std::fs::write(root.path().join("qualitygate.yaml"),
        "schema_version: 1\ncustom_rules: rules\nrules:\n  private-trailer: {}\n  ai-code-traceability:\n    parameters:\n      languages: [python, java]\n      marker: {type: git_trailer, name: AI-Generated, fields: [author, reason]}\n").unwrap();
    commit(root.path(), "policy");
    root
}

fn commit(root: &Path, message: &str) {
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", message]);
}

fn oid(root: &Path) -> String {
    let result = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(result.status.success());
    String::from_utf8(result.stdout).unwrap().trim().into()
}

fn check(root: &Path, args: &[&str], code: i32) -> Value {
    let mut argv = vec!["check", "--format", "json"];
    argv.extend_from_slice(args);
    report(&cli(root, &argv), code)
}

fn rule<'a>(output: &'a Value, id: &str) -> &'a Value {
    output["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["id"] == id)
        .unwrap_or_else(|| panic!("Missing {id}: {output}"))
}

fn assert_verdict(output: &Value, expected: &str) {
    for id in IDS {
        assert_eq!(rule(output, id)["verdict"], expected, "{output}");
    }
}

#[test]
fn unrelated_commit_trailers_cannot_cover_a_different_test_introduction() {
    let root = fixture_with_policy();
    let base = oid(root.path());
    std::fs::write(root.path().join("hello.txt"), "unrelated\n").unwrap();
    commit(root.path(), MARKED);
    std::fs::write(root.path().join("test_example.py"), PYTHON).unwrap();
    commit(root.path(), "tests without a declaration");
    let introduction = oid(root.path());
    let failed = check(root.path(), &["--base", &base], 1);
    assert_verdict(&failed, "fail");
    for id in IDS {
        let evidence = &rule(&failed, id)["metadata"]["git_trailers"];
        assert_eq!(evidence["entities"][0]["commits"], json!([introduction]));
        assert_eq!(evidence["declaration_only"], true);
    }
    std::fs::write(
        root.path().join("test_example.py"),
        PYTHON.replace("True", "1 == 1"),
    )
    .unwrap();
    commit(root.path(), MARKED);
    assert_verdict(&check(root.path(), &["--base", &base], 0), "pass");
}

#[test]
fn staged_and_worktree_entity_edits_cannot_borrow_committed_declarations() {
    let root = fixture_with_policy();
    std::fs::write(root.path().join("test_example.py"), PYTHON).unwrap();
    commit(root.path(), MARKED);
    std::fs::write(
        root.path().join("test_example.py"),
        PYTHON.replace("True", "False"),
    )
    .unwrap();
    let dirty = check(root.path(), &[], 1);
    assert_verdict(&dirty, "fail");
    assert_eq!(
        rule(&dirty, IDS[0])["metadata"]["git_trailers"]["entities"][0]["commits"],
        json!([null])
    );
    assert_verdict(&check(root.path(), &["--staged"], 0), "pass");
    git(root.path(), &["add", "test_example.py"]);
    assert_verdict(&check(root.path(), &["--staged"], 1), "fail");
    std::fs::write(root.path().join("test_example.py"), PYTHON).unwrap();
    assert_verdict(&check(root.path(), &[], 0), "pass");
    assert_verdict(&check(root.path(), &["--staged"], 1), "fail");
}

#[test]
fn only_real_unambiguous_trailers_with_complete_fields_satisfy_the_binding() {
    let root = fixture_with_policy();
    let base = oid(root.path());
    std::fs::write(root.path().join("test_example.py"), PYTHON).unwrap();
    commit(
        root.path(),
        "tests\n\nAI-Generated: author='fake' reason='body text'\n\nMore prose after the fake trailer.",
    );
    assert_verdict(&check(root.path(), &["--base", &base], 1), "fail");
    for (message, code) in [
        ("tests\n\nAI-Generated: author='agent'", 1),
        (
            "tests\n\nAI-Generated: author='agent' reason='one'\nAI-Generated: author='agent' reason='two'",
            2,
        ),
        (
            "tests\n\n---\nNotes before the footer.\n\naI-gEnErAtEd: author='agent'\n reason='folded declaration'",
            0,
        ),
    ] {
        git(root.path(), &["commit", "--amend", "-qm", message]);
        let result = check(root.path(), &["--base", &base], code);
        if code == 2 {
            for id in IDS {
                assert_eq!(rule(&result, id)["verdict"], Value::Null);
                assert!(
                    rule(&result, id)["execution"]["reason"]
                        .as_str()
                        .unwrap()
                        .contains("duplicate")
                );
            }
        }
    }
    git(root.path(), &["config", "trailer.fake.key", "AI-Generated"]);
    git(
        root.path(),
        &[
            "config",
            "trailer.fake.command",
            "echo author=forged reason=injected",
        ],
    );
    git(
        root.path(),
        &["commit", "--amend", "-qm", "tests without trailers"],
    );
    assert_verdict(&check(root.path(), &["--base", &base], 1), "fail");
}

#[test]
fn committed_moves_preserve_entity_origins_and_new_copies_need_their_own_commit() {
    let root = fixture_with_policy();
    let base = oid(root.path());
    std::fs::write(root.path().join("test_example.py"), PYTHON).unwrap();
    commit(root.path(), MARKED);
    let introduction = oid(root.path());
    git(root.path(), &["mv", "test_example.py", "test_moved.py"]);
    assert_verdict(&check(root.path(), &["--base", &base], 1), "fail");
    commit(root.path(), "move without changing test bytes");
    let moved = check(root.path(), &["--base", &base], 0);
    assert_verdict(&moved, "pass");
    for id in IDS {
        assert_eq!(
            rule(&moved, id)["metadata"]["git_trailers"]["entities"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            rule(&moved, id)["metadata"]["git_trailers"]["entities"][0]["commits"],
            json!([introduction])
        );
    }
    std::fs::write(root.path().join("test_copy.py"), PYTHON).unwrap();
    commit(root.path(), "copy without a declaration");
    let copied = check(root.path(), &["--base", &base], 1);
    for id in IDS {
        assert_eq!(
            rule(&copied, id)["diagnostics"].as_array().unwrap().len(),
            1
        );
        assert_eq!(rule(&copied, id)["diagnostics"][0]["file"], "test_copy.py");
    }
}

#[test]
fn merges_follow_unchanged_side_parent_entities_and_attribute_resolution_edits() {
    let root = fixture_with_policy();
    let base = oid(root.path());
    git(root.path(), &["checkout", "-qb", "side"]);
    std::fs::write(root.path().join("test_example.py"), PYTHON).unwrap();
    commit(root.path(), MARKED);
    let side = oid(root.path());
    git(root.path(), &["checkout", "-qb", "mainline", &base]);
    std::fs::write(root.path().join("hello.txt"), "mainline\n").unwrap();
    commit(root.path(), "unrelated mainline work");
    git(
        root.path(),
        &["merge", "--no-ff", "-qm", "merge side tests", "side"],
    );
    let merged = check(root.path(), &["--base", &base], 0);
    assert_eq!(
        rule(&merged, IDS[0])["metadata"]["git_trailers"]["entities"][0]["commits"],
        json!([side])
    );
    let fork = oid(root.path());
    git(root.path(), &["checkout", "-qb", "left"]);
    std::fs::write(
        root.path().join("test_example.py"),
        PYTHON.replace("True", "False"),
    )
    .unwrap();
    commit(root.path(), MARKED);
    git(root.path(), &["checkout", "-qb", "right", &fork]);
    std::fs::write(
        root.path().join("test_example.py"),
        PYTHON.replace("True", "1 == 1"),
    )
    .unwrap();
    commit(root.path(), MARKED);
    let conflict = Command::new("git")
        .args(["merge", "--no-ff", "--no-edit", "left"])
        .current_dir(root.path())
        .output()
        .unwrap();
    assert!(!conflict.status.success());
    std::fs::write(
        root.path().join("test_example.py"),
        PYTHON.replace("True", "2 == 2"),
    )
    .unwrap();
    commit(root.path(), "resolve test conflict");
    assert_verdict(&check(root.path(), &["--base", &fork], 1), "fail");
    git(root.path(), &["commit", "--amend", "-qm", MARKED]);
    assert_verdict(&check(root.path(), &["--base", &fork], 0), "pass");
}

#[test]
fn shallow_ancestry_is_incomplete_and_replace_refs_cannot_forge_commit_evidence() {
    let root = fixture_with_policy();
    let base = oid(root.path());
    std::fs::write(root.path().join("test_example.py"), PYTHON).unwrap();
    commit(root.path(), "unmarked test");
    let original = oid(root.path());
    git(root.path(), &["commit", "--amend", "-qm", MARKED]);
    let replacement = oid(root.path());
    git(root.path(), &["reset", "--hard", &original]);
    git(root.path(), &["replace", &original, &replacement]);
    assert_verdict(&check(root.path(), &["--base", &base], 1), "fail");
    git(root.path(), &["replace", "-d", &original]);
    let parent = tempfile::tempdir().unwrap();
    let cloned = parent.path().join("shallow");
    git(
        parent.path(),
        &[
            "clone",
            "--quiet",
            "--no-local",
            "--depth",
            "1",
            root.path().to_str().unwrap(),
            cloned.to_str().unwrap(),
        ],
    );
    let incomplete = check(&cloned, &[], 2);
    for id in IDS {
        assert_eq!(rule(&incomplete, id)["verdict"], Value::Null);
        assert!(
            rule(&incomplete, id)["execution"]["reason"]
                .as_str()
                .unwrap()
                .contains("ancestry")
        );
    }
}
