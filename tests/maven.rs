//! Live Maven acceptance. Run explicitly in the Maven CI job; ordinary Rust tests
//! use parser fixtures and do not silently substitute a mock for this evidence.

mod common;
use common::*;
use serde_json::{Value, json};
use std::path::Path;

const SOURCE: &str =
    "# Test declarations\nRetain test declarations and their declared JUnit dependency.\n";
const DEPENDENCY: &str = "<dependencies><dependency><groupId>junit</groupId><artifactId>junit</artifactId><version>4.13.2</version><scope>test</scope></dependency></dependencies>";

fn pom(root: &Path, dependency: &str) {
    std::fs::write(root.join("pom.xml"), format!("<project><modelVersion>4.0.0</modelVersion><groupId>fixture</groupId><artifactId>sample</artifactId><version>1.0</version>{dependency}</project>")).unwrap();
}

fn configure(root: &Path, maven: &str, cache: &Path) {
    std::fs::create_dir_all(root.join("rules")).unwrap();
    std::fs::create_dir_all(root.join("src/test/java")).unwrap();
    std::fs::write(root.join("AGENTS.md"), SOURCE).unwrap();
    std::fs::write(root.join(".gitignore"), "target/\n").unwrap();
    std::fs::write(root.join("rules/pair.yaml"), format!(
        "id: paired\nversion: 1\nsource: {{document: AGENTS.md, section: Test declarations, content_hash: {}}}\nlanguage: [java]\napplies_to: {{paths: ['src/test/**/*.java'], provenance_scope: all_added_tests}}\nrequires_capabilities: [test_methods, annotations, dependency_resolution]\nbinding: {{marker: {{type: annotation, name: Generated, fields: [author]}}}}\nwhen: {{entity: test_method, change: added}}\nthen: {{require_marker: true, require_dependency: {{group: junit, artifact: junit}}}}\nfix: Restore the honest declaration and the test dependency; rerun\n", qualitygate::snapshot::digest(SOURCE.as_bytes()))).unwrap();
    let config = json!({"schema_version":1, "custom_rules":"rules", "rules":{"paired":{"depends_on":["maven-facts"]}},
        "checks":[{"id":"maven-facts", "argv":[maven,"-B","-ntp","-N",format!("-Dmaven.repo.local={}",cache.display()),
            "org.apache.maven.plugins:maven-help-plugin:3.5.1:effective-pom", "-Doutput=target/effective.xml",
            "org.apache.maven.plugins:maven-dependency-plugin:3.8.1:tree", "-DoutputType=json", "-DoutputFile=target/tree.json"],
            "timeout_seconds":300, "tools":[{"id":"maven", "argv":[maven,"--version"]}, {"id":"java", "argv":["java","-version"]}],
            "projects":[{"root":".","effective_pom":"target/effective.xml","dependency_tree":"target/tree.json"}]}],
        "profiles":{"quick":{"include":["paired"]}}});
    std::fs::write(
        root.join("qualitygate.yaml"),
        serde_norway::to_string(&config).unwrap(),
    )
    .unwrap();
    pom(root, DEPENDENCY);
    std::fs::write(
        root.join("src/test/java/T.java"),
        "class T { @Test @Generated(author=\"fixture\") void test() {} }\n",
    )
    .unwrap();
}

fn run(root: &Path, code: i32) -> Value {
    report(
        &cli(root, &["check", "--profile", "quick", "--format", "json"]),
        code,
    )
}

#[test]
#[ignore = "requires a real Maven installation, JDK, and artifact access; mandatory Maven CI job"]
fn real_maven_dependency_pairing_repair_and_retention() {
    let maven = std::env::var("QUALITYGATE_TEST_MAVEN").unwrap_or_else(|_| "mvn".into());
    let cache = tempfile::tempdir().unwrap();
    let root = fixture();
    let root = root.path();
    configure(root, &maven, cache.path());
    let passed = run(root, 0);
    assert_eq!(
        passed["plan"]["execution_order"],
        json!(["maven-facts", "paired"])
    );
    let facts = &passed["checks"][0]["metadata"]["projects"][0];
    assert_eq!(facts["coordinate"], "fixture:sample:1.0");
    assert_eq!(facts["test_source_root"], "src/test/java");
    assert_eq!(
        facts["snapshot_digest"],
        passed["snapshot"]["content_digest"]
    );
    assert_eq!(facts["declared"][0]["artifact"], "junit");
    assert!(
        facts["resolved"]
            .as_array()
            .unwrap()
            .iter()
            .any(|dependency| dependency["artifact"] == "hamcrest-core")
    );
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "working dependency pairing"]);
    pom(root, "");
    let missing = run(root, 1);
    assert_eq!(
        missing["checks"][1]["metadata"]["dependency_scope"]["tests"],
        1
    );
    assert_eq!(
        missing["checks"][1]["diagnostics"][0]["evidence"]["assertion"],
        "require_dependency"
    );
    pom(root, DEPENDENCY);
    run(root, 0);
    std::fs::write(
        root.join("src/test/java/T.java"),
        "class T { @Test void test() {} }\n",
    )
    .unwrap();
    let removed = run(root, 1);
    assert_eq!(
        removed["checks"][1]["diagnostics"][0]["evidence"]["assertion"],
        "require_marker"
    );
    pom(
        root,
        &DEPENDENCY.replace("<scope>test</scope>", "<scope>runtime</scope>"),
    );
    let runtime_only = run(root, 1);
    assert_eq!(
        runtime_only["checks"][1]["diagnostics"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    // Captured raw reports are durable and reference the actual Maven invocation.
    for artifact in passed["checks"][0]["execution"]["artifacts"]
        .as_array()
        .unwrap()
    {
        let bytes = std::fs::read(artifact["path"].as_str().unwrap()).unwrap();
        assert_eq!(qualitygate::snapshot::digest(&bytes), artifact["digest"]);
    }
}
