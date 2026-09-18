mod common;

use common::{cli, fixture, report};
use serde_json::Value;
use std::{fs, path::Path};

const CASES: &[(&str, &str, &str, &str)] = &[
    (
        "go-sql-injection",
        "error",
        "fmt.Sprintf(\"SELECT * FROM users WHERE id=%d\", id)\n",
        "db.Query(\"SELECT * FROM users WHERE id=?\", id)\n",
    ),
    (
        "go-insecure-randomness",
        "warning",
        "import \"math/rand\"\n",
        "import \"crypto/rand\"\n",
    ),
    (
        "go-tls-insecure-skip-verify",
        "error",
        "tls.Config{InsecureSkipVerify: true}\n",
        "tls.Config{MinVersion: tls.VersionTLS12}\n",
    ),
    (
        "go-hardcoded-credentials",
        "error",
        "password := \"secret123\"\n",
        "password := os.Getenv(\"PASSWORD\")\n",
    ),
    (
        "go-ssh-insecure-ignore-host-key",
        "error",
        "ssh.InsecureIgnoreHostKey()\n",
        "ssh.FixedHostKey(key)\n",
    ),
    (
        "go-file-permission-creation",
        "warning",
        "os.WriteFile(path, data, 0o666)\n",
        "os.WriteFile(path, data, 0o600)\n",
    ),
    (
        "go-panic-in-exported-function",
        "warning",
        "panic(err)\n",
        "return err\n",
    ),
    (
        "go-cgo-cstring-without-defer-free",
        "warning",
        "ptr := C.CString(name)\n",
        "ptr := []byte(name)\n",
    ),
    (
        "go-relative-import-path",
        "warning",
        "import \"../other\"\n",
        "import \"example.com/project/other\"\n",
    ),
    (
        "go-dot-import",
        "warning",
        "import . \"fmt\"\n",
        "import \"fmt\"\n",
    ),
    (
        "go-sensitive-info-in-log",
        "warning",
        "log.Printf(\"password=%s\", password)\n",
        "log.Printf(\"user=%s\", user)\n",
    ),
    (
        "go-float-loop-counter",
        "warning",
        "for i := 0.0; i < 10; i++ { }\n",
        "for i := 0; i < 10; i++ { }\n",
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
fn go_rules_are_discoverable_opt_in_and_reject_scope_drift() {
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
                "go",
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
            serde_json::json!(["go"])
        );
        assert_eq!(
            entry["definition"]["builtin"]["defaults"]["severity"],
            *severity
        );
        assert_eq!(entry["definition"]["package"], "lang-go");
        assert_eq!(entry["enabled"], false);
    }
    for invalid in [
        "schema_version: 1\nrulesets: [lang-go]\nrules: {go-sql-injection: {parameters: {languages: [python]}}}\n",
        "schema_version: 1\nrulesets: [lang-go]\nrules: {go-dot-import: {parameters: {prohibited_patterns: {python: ['import']}}}}\n",
        "schema_version: 1\nrulesets: [lang-go]\nrules: {go-float-loop-counter: {parameters: {prohibited_patterns: {go: ['[']}}}}\n",
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
fn go_patterns_find_changed_go_lines_then_repair() {
    let repository = fixture();
    let root = repository.path();
    fs::create_dir(root.join("src")).unwrap();
    for (id, severity, bad, good) in CASES {
        fs::write(
            root.join("qualitygate.yaml"),
            format!("schema_version: 1\nrulesets: [lang-go]\nrules: {{{id}: {{}}}}\n"),
        )
        .unwrap();
        fs::write(root.join("src/sample.go"), bad).unwrap();
        fs::write(root.join("src/other.py"), bad).unwrap();
        let failed = check(root, if *severity == "error" { 1 } else { 0 });
        assert_eq!(
            rule(&failed, id)["diagnostics"].as_array().unwrap().len(),
            1,
            "{id}"
        );
        assert_eq!(
            rule(&failed, id)["diagnostics"][0]["evidence"]["language"],
            "go"
        );
        fs::write(root.join("src/sample.go"), good).unwrap();
        assert_eq!(
            rule(&check(root, 0), id)["diagnostics"]
                .as_array()
                .unwrap()
                .len(),
            0,
            "{id}"
        );
    }
    fs::write(
        root.join("qualitygate.yaml"),
        "schema_version: 1\nrulesets: [lang-go]\nrules: {go-sql-injection: {}}\n",
    )
    .unwrap();
    fs::write(
        root.join("src/sample.go"),
        "query := \"SELECT * FROM users WHERE id=\" + userID\n",
    )
    .unwrap();
    assert_eq!(
        rule(&check(root, 1), "go-sql-injection")["diagnostics"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}
