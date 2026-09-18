mod common;

use common::{cli, fixture, report};
use serde_json::Value;
use std::{fs, path::Path};

const CASES: &[(&str, &str, &str, &str)] = &[
    (
        "py-eval-exec",
        "error",
        "eval(user_input)\n",
        "parse(user_input)\n",
    ),
    (
        "py-shell-equals-true",
        "error",
        "subprocess.run(command, shell=True)\n",
        "subprocess.run(argv, shell=False)\n",
    ),
    (
        "py-insecure-randomness",
        "warning",
        "token = random.randint(0, 100)\n",
        "token = secrets.randbelow(100)\n",
    ),
    (
        "py-tls-verify-disabled",
        "error",
        "requests.get(url, verify=False)\n",
        "requests.get(url, verify=True)\n",
    ),
    (
        "py-yaml-unsafe-load",
        "error",
        "config = yaml.load(text)\n",
        "config = yaml.safe_load(text)\n",
    ),
    (
        "py-sql-string-format",
        "error",
        "cursor.execute(f\"SELECT * FROM users WHERE id={user_id}\")\n",
        "cursor.execute(\"SELECT * FROM users WHERE id=?\", (user_id,))\n",
    ),
    (
        "py-hardcoded-credentials",
        "error",
        "password = \"secret123\"\n",
        "password = os.environ[\"PASSWORD\"]\n",
    ),
    (
        "py-tempfile-mktemp",
        "error",
        "path = tempfile.mktemp()\n",
        "path = tempfile.mkstemp()\n",
    ),
    (
        "py-bare-except",
        "warning",
        "except:\n",
        "except ValueError:\n",
    ),
    (
        "py-mutable-default-argument",
        "warning",
        "def collect(items=[]): pass\n",
        "def collect(items=None): pass\n",
    ),
    (
        "py-assert-in-production",
        "warning",
        "assert authorized\n",
        "if not authorized: raise PermissionError()\n",
    ),
    (
        "py-sensitive-info-in-log",
        "warning",
        "logger.info(\"password=%s\", password)\n",
        "logger.info(\"user=%s\", user)\n",
    ),
    (
        "py-pickle-load",
        "error",
        "value = pickle.loads(data)\n",
        "value = json.loads(data)\n",
    ),
];

fn check(root: &Path, expected: i32) -> Value {
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
        expected,
    )
}

fn rule<'a>(report: &'a Value, id: &str) -> &'a Value {
    report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["id"] == id)
        .unwrap()
}

#[test]
fn python_rules_are_discoverable_opt_in_and_reject_scope_drift() {
    let repository = fixture();
    let root = repository.path();
    let list = report(
        &cli(
            root,
            &[
                "rules",
                "list",
                "--source",
                "builtin",
                "--language",
                "python",
                "--format",
                "json",
            ],
        ),
        0,
    );
    for (id, severity, _, _) in CASES {
        let entry = list["rules"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["id"] == *id)
            .unwrap();
        assert_eq!(
            entry["definition"]["builtin"]["language"],
            serde_json::json!(["python"])
        );
        assert_eq!(
            entry["definition"]["builtin"]["defaults"]["severity"],
            *severity
        );
        assert_eq!(entry["definition"]["package"], "lang-python");
        assert_eq!(entry["enabled"], false);
    }
    for invalid in [
        "schema_version: 1\nrulesets: [lang-python]\nrules: {py-eval-exec: {parameters: {languages: [go]}}}\n",
        "schema_version: 1\nrulesets: [lang-python]\nrules: {py-bare-except: {parameters: {prohibited_patterns: {go: ['except:']}}}}\n",
        "schema_version: 1\nrulesets: [lang-python]\nrules: {py-pickle-load: {parameters: {prohibited_patterns: {python: ['[']}}}}\n",
    ] {
        fs::write(root.join("qualitygate.yaml"), invalid).unwrap();
        assert_eq!(
            cli(root, &["config", "--show", "--format", "json"])
                .status
                .code(),
            Some(2)
        );
    }
}

#[test]
fn python_patterns_find_changed_lines_then_repair() {
    let repository = fixture();
    let root = repository.path();
    fs::create_dir(root.join("src")).unwrap();
    for (id, severity, bad, good) in CASES {
        fs::write(
            root.join("qualitygate.yaml"),
            format!("schema_version: 1\nrulesets: [lang-python]\nrules: {{{id}: {{}}}}\n"),
        )
        .unwrap();
        fs::write(root.join("src/sample.py"), bad).unwrap();
        fs::write(root.join("src/other.go"), bad).unwrap();
        let failed = check(root, if *severity == "error" { 1 } else { 0 });
        assert_eq!(
            rule(&failed, id)["diagnostics"].as_array().unwrap().len(),
            1,
            "{id}"
        );
        assert_eq!(
            rule(&failed, id)["diagnostics"][0]["evidence"]["language"],
            "python"
        );
        fs::write(root.join("src/sample.py"), good).unwrap();
        assert_eq!(
            rule(&check(root, 0), id)["diagnostics"]
                .as_array()
                .unwrap()
                .len(),
            0,
            "{id}"
        );
    }
}
