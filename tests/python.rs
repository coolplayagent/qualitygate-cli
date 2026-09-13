//! Live pip installation and pytest repair acceptance; never replaced by mocks.

mod common;
#[path = "common/reviews.rs"]
mod reviews;
use common::*;
use serde_json::{Value, json};
use std::path::Path;

const SOURCE: &str = "# Test dependencies\nMarked tests retain their declared pytest dependency.\n";

fn manifest(dependency: &str) -> String {
    format!(
        "[build-system]\nrequires=['setuptools==80.9.0']\nbuild-backend='setuptools.build_meta'\n[project]\nname='qualitygate-python-fixture'\nversion='1.0'\nrequires-python='>=3.10'\n[project.optional-dependencies]\ntest=[{dependency}]\n"
    )
}

fn configure(root: &Path, python: &str) {
    std::fs::create_dir_all(root.join("src/qg_fixture")).unwrap();
    std::fs::create_dir_all(root.join("tests")).unwrap();
    std::fs::create_dir_all(root.join("rules")).unwrap();
    std::fs::write(
        root.join("src/qg_fixture/__init__.py"),
        "def add(a, b):\n    return a + b\n",
    )
    .unwrap();
    std::fs::write(root.join("tests/test_add.py"),"from qg_fixture import add\n\n# Generated author=fixture\ndef test_add():\n    assert add(1, 2) == 3\n").unwrap();
    std::fs::write(root.join("pyproject.toml"), manifest("'pytest==8.4.2'")).unwrap();
    std::fs::write(root.join("AGENTS.md"), SOURCE).unwrap();
    std::fs::write(
        root.join(".gitignore"),
        "target/\n*.egg-info/\n__pycache__/\n",
    )
    .unwrap();
    std::fs::write(root.join("rules/paired.yaml"),format!("id: paired\nversion: 1\nsource: {{document: AGENTS.md,section: Test dependencies,content_hash: {}}}\nlanguage: [python]\nrequires_capabilities: [test_methods,comments,dependency_resolution]\napplies_to: {{paths: ['tests/**/*.py'],provenance_scope: all_added_tests}}\nbinding: {{marker: {{type: comment,name: Generated,fields: [author]}}}}\nwhen: {{entity: test_method,change: added}}\nthen: {{require_marker: true,require_dependency: {{artifact: pytest}}}}\nfix: Restore the honest declaration and pytest dependency, then run the test\n",qualitygate::snapshot::digest(SOURCE.as_bytes()))).unwrap();
    std::fs::write(
        root.join("pip.py"),
        "raise RuntimeError('Repository pip.py must not replace the installation tool')\n",
    )
    .unwrap();
    let tools = json!([{"id":"python","argv":[python,"-E","-P","--version"]},{"id":"pip","argv":[python,"-E","-P","-m","pip","--version"]}]);
    let mut test_tools = tools.clone();
    test_tools.as_array_mut().unwrap().push(json!({"id":"pytest","argv":["env","PYTHONPATH=target/python",python,"-P","-m","pytest","--version"]}));
    let mut config = json!({"schema_version":1,"custom_rules":"rules","rules":{"paired":{"depends_on":["python-facts"]}},"checks":[
        {"id":"python-facts","argv":[python,"-E","-P","-m","pip","--isolated","install","--ignore-installed","--no-compile","--disable-pip-version-check","--no-warn-script-location","--target","target/python","--report","target/pip.json",".[test]"],"timeout_seconds":300,"tools":tools,"projects":[{"ecosystem":"python","root":".","source_root":"src","test_source_root":"tests","install_target":"target/python","install_report":"target/pip.json","extras":["test"]}]},
        {"id":"tests","depends_on":["paired"],"argv":["env","PYTHONPATH=target/python",python,"-P","-m","pytest","-q","--junitxml=target/junit.xml","tests"],"timeout_seconds":120,"findings_exit_codes":[1],"tools":test_tools,"reports":[{"path":"target/junit.xml","format":"junit","minimum_tests":1}]}
    ],"profiles":{"quick":{"include":["tests"]}}});
    config["checks"][0]["argv"]
        .as_array_mut()
        .unwrap()
        .push(json!("--no-cache-dir"));
    std::fs::write(
        root.join("qualitygate.yaml"),
        serde_norway::to_string(&config).unwrap(),
    )
    .unwrap();
    reviews::record(root).unwrap();
}

fn run(root: &Path, code: i32) -> Value {
    let output = cli(root, &["check", "--profile", "quick", "--format", "json"]);
    if output.status.code() != Some(code)
        && let Ok(value) = serde_json::from_slice::<Value>(&output.stdout)
    {
        for check in value["checks"].as_array().unwrap() {
            for artifact in check["execution"]["artifacts"].as_array().unwrap() {
                let path = artifact["path"].as_str().unwrap();
                if path.ends_with(".log") {
                    let bytes = std::fs::read(path).unwrap();
                    eprintln!(
                        "{path}: {}",
                        String::from_utf8_lossy(&bytes[bytes.len().saturating_sub(8192)..])
                    );
                }
            }
        }
    }
    report(&output, code)
}

#[test]
#[ignore = "requires real Python/pip, pytest and artifact access; mandatory Python CI job"]
fn real_python_extra_dependency_marker_retention_and_test_repair() {
    let python = std::env::var("QUALITYGATE_TEST_PYTHON").unwrap_or_else(|_| "python3".into());
    let root = fixture();
    let root = root.path();
    configure(root, &python);
    let passed = run(root, 0);
    assert_eq!(
        passed["checks"][0]["metadata"]["projects"][0]["ecosystem"],
        "python"
    );
    assert_eq!(
        passed["checks"][0]["metadata"]["projects"][0]["declared"][0]["artifact"],
        "pytest"
    );
    assert_eq!(passed["checks"][2]["verdict"], "pass");
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "verified Python test dependency"]);
    std::fs::write(root.join("pyproject.toml"), manifest("")).unwrap();
    let missing = run(root, 2);
    assert_eq!(missing["checks"][1]["verdict"], "fail");
    assert_eq!(missing["checks"][2]["execution"]["status"], "blocked");
    std::fs::write(root.join("pyproject.toml"), manifest("'pytest==8.4.2'")).unwrap();
    std::fs::write(
        root.join("src/qg_fixture/__init__.py"),
        "def add(a, b):\n    return a - b\n",
    )
    .unwrap();
    let broken = run(root, 1);
    assert_eq!(broken["checks"][1]["verdict"], "pass");
    assert_eq!(broken["checks"][2]["verdict"], "fail");
    std::fs::write(
        root.join("src/qg_fixture/__init__.py"),
        "def add(a, b):\n    return a + b\n",
    )
    .unwrap();
    run(root, 0);
    std::fs::write(
        root.join("tests/test_add.py"),
        "from qg_fixture import add\n\ndef test_add():\n    assert add(1, 2) == 3\n",
    )
    .unwrap();
    let removed = run(root, 2);
    assert_eq!(
        removed["checks"][1]["diagnostics"][0]["evidence"]["assertion"],
        "require_marker"
    );
}
