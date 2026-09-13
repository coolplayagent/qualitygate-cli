//! Live Maven acceptance. Run explicitly in the Maven CI job; ordinary Rust tests
//! use parser fixtures and do not silently substitute a mock for this evidence.

mod common;
#[path = "common/reviews.rs"]
mod reviews;
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
    reviews::record(root).unwrap();
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

#[test]
#[ignore = "requires real Maven, JDK and artifact access; mandatory Maven CI job"]
fn real_reactor_dependency_direction_requires_compiled_repair() {
    let maven = std::env::var("QUALITYGATE_TEST_MAVEN").unwrap_or_else(|_| "mvn".into());
    let cache = tempfile::tempdir().unwrap();
    let root = fixture();
    let root = root.path();
    std::fs::write(root.join(".gitignore"), "target/\n").unwrap();
    std::fs::write(root.join("pom.xml"), "<project><modelVersion>4.0.0</modelVersion><groupId>fixture</groupId><artifactId>reactor</artifactId><version>1.0</version><packaging>pom</packaging><modules><module>infra</module><module>core</module></modules><properties><maven.compiler.source>17</maven.compiler.source><maven.compiler.target>17</maven.compiler.target></properties></project>").unwrap();
    let module = |name: &str, dependency: bool| {
        format!(
            "<project><modelVersion>4.0.0</modelVersion><parent><groupId>fixture</groupId><artifactId>reactor</artifactId><version>1.0</version></parent><artifactId>{name}</artifactId>{}</project>",
            if dependency {
                "<dependencies><dependency><groupId>fixture</groupId><artifactId>infra</artifactId><version>1.0</version></dependency></dependencies>"
            } else {
                ""
            }
        )
    };
    for name in ["infra", "core"] {
        std::fs::create_dir_all(root.join(format!("{name}/src/main/java/fixture"))).unwrap();
        std::fs::write(
            root.join(format!("{name}/pom.xml")),
            module(name, name == "core"),
        )
        .unwrap();
    }
    std::fs::write(root.join("infra/src/main/java/fixture/Infra.java"), "package fixture; public class Infra { public static String value() { return \"value\"; } }\n").unwrap();
    std::fs::write(
        root.join("core/src/main/java/fixture/Core.java"),
        "package fixture; public class Core { public String value() { return Infra.value(); } }\n",
    )
    .unwrap();
    let cache_arg = format!("-Dmaven.repo.local={}", cache.path().display());
    let tools =
        json!([{"id":"maven","argv":[maven,"--version"]},{"id":"java","argv":["java","-version"]}]);
    let mut checks = vec![
        json!({"id":"reactor-build","argv":[maven,"-B","-ntp",cache_arg,"-DskipTests","install"], "timeout_seconds":600, "tools":tools}),
    ];
    for name in ["core", "infra"] {
        checks.push(json!({"id":format!("facts-{name}"), "cwd":name, "depends_on":["reactor-build"], "argv":[maven,"-B","-ntp","-N",cache_arg,
            "org.apache.maven.plugins:maven-help-plugin:3.5.1:effective-pom","-Doutput=target/effective.xml",
            "org.apache.maven.plugins:maven-dependency-plugin:3.8.1:tree","-DoutputType=json","-DoutputFile=target/tree.json"],
            "timeout_seconds":300, "tools":tools, "projects":[{"root":name, "effective_pom":format!("{name}/target/effective.xml"), "dependency_tree":format!("{name}/target/tree.json")}]}));
    }
    let config = json!({"schema_version":1, "rulesets":["lang-java"], "rules":{"module-boundary":{"depends_on":["facts-core","facts-infra"],
        "parameters":{"modules":["core","infra"],"forbidden":[{"from":"fixture:core","to":"fixture:infra","scopes":["compile"]}]}}},
        "checks":checks, "profiles":{"quick":{"include":["module-boundary"]}}});
    std::fs::write(
        root.join("qualitygate.yaml"),
        serde_norway::to_string(&config).unwrap(),
    )
    .unwrap();
    let failed = run(root, 1);
    assert_eq!(
        failed["plan"]["execution_order"],
        json!([
            "reactor-build",
            "facts-core",
            "facts-infra",
            "module-boundary"
        ])
    );
    assert_eq!(
        failed["checks"][3]["diagnostics"][0]["evidence"]["from"],
        "fixture:core"
    );
    assert_eq!(
        failed["checks"][3]["diagnostics"][0]["evidence"]["to"],
        "fixture:infra"
    );
    assert_eq!(failed["checks"][0]["verdict"], "pass");
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "existing boundary violation"]);
    // Removing only the manifest edge cannot hide unresolved caller code.
    std::fs::write(root.join("core/pom.xml"), module("core", false)).unwrap();
    let broken = run(root, 2);
    assert_eq!(broken["checks"][0]["verdict"], "fail");
    assert_eq!(broken["checks"][3]["execution"]["status"], "blocked");
    std::fs::write(
        root.join("core/src/main/java/fixture/Core.java"),
        "package fixture; public class Core { public String value() { return \"value\"; } }\n",
    )
    .unwrap();
    let repaired = run(root, 0);
    assert_eq!(repaired["checks"][3]["verdict"], "pass");
    assert_eq!(
        repaired["checks"][3]["metadata"]["module_inventory"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[test]
#[ignore = "requires real Maven, JDK and artifact access; mandatory Maven CI job"]
fn real_used_transitive_dependency_requires_direct_declaration() {
    let maven = std::env::var("QUALITYGATE_TEST_MAVEN").unwrap_or_else(|_| "mvn".into());
    let cache = tempfile::tempdir().unwrap();
    let root = fixture();
    let root = root.path();
    std::fs::create_dir_all(root.join("src/test/java")).unwrap();
    std::fs::create_dir_all(root.join("src/main/java")).unwrap();
    std::fs::write(
        root.join("src/main/java/Main.java"),
        "public class Main { public String value() { return \"value\"; } }\n",
    )
    .unwrap();
    std::fs::write(root.join(".gitignore"), "target/\n").unwrap();
    let model = |dependencies: &str| {
        format!(
            "<project><modelVersion>4.0.0</modelVersion><groupId>fixture</groupId><artifactId>sample</artifactId><version>1</version><properties><maven.compiler.source>17</maven.compiler.source><maven.compiler.target>17</maven.compiler.target></properties>{dependencies}</project>"
        )
    };
    std::fs::write(root.join("pom.xml"), model(DEPENDENCY)).unwrap();
    std::fs::write(root.join("src/test/java/Usage.java"),"import org.hamcrest.Matcher; import org.hamcrest.CoreMatchers; class Usage { Matcher<String> value() { return CoreMatchers.is(\"value\"); } }\n").unwrap();
    let mut argv = vec![
        maven.clone(),
        "-B".into(),
        "-ntp".into(),
        format!("-Dmaven.repo.local={}", cache.path().display()),
    ];
    argv.extend(
        qualitygate::config::project_rules::MAVEN_USAGE_ARGS
            .iter()
            .map(|arg| (*arg).to_owned()),
    );
    argv.extend(
        [
            "clean",
            "test-compile",
            "org.apache.maven.plugins:maven-help-plugin:3.5.1:effective-pom",
            "-Doutput=target/effective.xml",
            "org.apache.maven.plugins:maven-dependency-plugin:3.8.1:tree",
            "-DoutputType=json",
            "-DoutputFile=target/tree.json",
            qualitygate::config::project_rules::MAVEN_USAGE_GOAL,
        ]
        .map(str::to_owned),
    );
    let config = json!({"schema_version":1,"rulesets":["lang-java"],"rules":{"used-undeclared":{"depends_on":["facts"],"parameters":{"modules":["."]}}},"checks":[{"id":"facts","argv":argv,"timeout_seconds":600,"tools":[{"id":"maven","argv":[maven,"--version"]},{"id":"java","argv":["java","-version"]}],"projects":[{"root":".","effective_pom":"target/effective.xml","dependency_tree":"target/tree.json","dependency_usage":true}]}],"profiles":{"quick":{"include":["used-undeclared"]}}});
    std::fs::write(
        root.join("qualitygate.yaml"),
        serde_norway::to_string(&config).unwrap(),
    )
    .unwrap();
    let missing = run(root, 1);
    assert_eq!(missing["checks"][0]["verdict"], "pass");
    assert_eq!(
        missing["checks"][1]["diagnostics"][0]["evidence"]["dependency"]["artifact"],
        "hamcrest-core"
    );
    assert_eq!(
        missing["checks"][0]["metadata"]["projects"][0]["dependency_usage"]["compiled_test_sources"],
        1
    );
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "compiled undeclared use"]);
    // A POM suppression must not turn the same undeclared bytecode use into a pass.
    let suppressed = model(DEPENDENCY).replace("</project>","<build><plugins><plugin><groupId>org.apache.maven.plugins</groupId><artifactId>maven-dependency-plugin</artifactId><version>3.8.1</version><configuration><ignoredUsedUndeclaredDependencies><ignoredUsedUndeclaredDependency>org.hamcrest:hamcrest-core</ignoredUsedUndeclaredDependency></ignoredUsedUndeclaredDependencies></configuration></plugin></plugins></build></project>");
    std::fs::write(root.join("pom.xml"), suppressed).unwrap();
    let suppressed = run(root, 2);
    assert!(
        suppressed["checks"][0]["execution"]["reason"]
            .as_str()
            .unwrap()
            .contains("may hide usage")
    );
    assert_eq!(suppressed["checks"][1]["execution"]["status"], "blocked");
    let declared = DEPENDENCY.replace("</dependencies>","<dependency><groupId>org.hamcrest</groupId><artifactId>hamcrest-core</artifactId><version>1.3</version><scope>test</scope></dependency></dependencies>");
    std::fs::write(root.join("pom.xml"), model(&declared)).unwrap();
    let repaired = run(root, 0);
    assert_eq!(
        repaired["checks"][0]["metadata"]["projects"][0]["dependency_usage"]["compiled_main_sources"],
        1
    );
    assert_eq!(repaired["checks"][1]["verdict"], "pass");
    // Deleting the only provider also breaks compilation, which cannot erase the check.
    std::fs::write(root.join("pom.xml"), model("")).unwrap();
    let broken = run(root, 2);
    assert_eq!(broken["checks"][1]["execution"]["status"], "blocked");
}
