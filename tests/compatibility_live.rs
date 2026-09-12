//! Real javac/JAR/japicmp acceptance, separate from deterministic Rust driver fixtures.
mod common;
use common::*;
use serde_json::{Value, json};
use std::{path::Path, process::Command, time::Duration};

const URL: &str = "https://github.com/siom79/japicmp/releases/download/japicmp-base-0.26.2/japicmp-0.26.2-jar-with-dependencies.jar";
const DIGEST: &str = "sha256:6c65dc29f205fdf57ea28255b901d817321fc1eda6e56afe266f36979a616466";
const API: &str = "public class Api extends Base { public String greet(String value) { return value; } public java.util.List<String> values() { return java.util.Collections.emptyList(); } }\n";
const BUILDER: &str = r#"fn main() {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--version") { println!("Java API fixture builder 1"); return; }
    std::fs::create_dir_all("target/classes").unwrap();
    for (program,argv) in [
        (&args[1],vec!["-g","-d","target/classes","Api.java","Base.java"]),
        (&args[2],vec!["--create","--file","target/api.jar","-C","target/classes","Api.class"]),
        (&args[2],vec!["--create","--file","target/dependency.jar","-C","target/classes","Base.class"]),
    ] {
        let status=std::process::Command::new(program).args(argv).status().unwrap();
        if !status.success() { std::process::exit(status.code().unwrap_or(1)); }
    }
}"#;

fn analyzer(directory: &Path) -> std::path::PathBuf {
    let bytes = if let Ok(path) = std::env::var("QUALITYGATE_TEST_JAPICMP") {
        std::fs::read(path).unwrap()
    } else {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let client = reqwest::Client::builder()
                    .timeout(Duration::from_secs(60))
                    .build()
                    .unwrap();
                let mut response = client
                    .get(URL)
                    .send()
                    .await
                    .unwrap()
                    .error_for_status()
                    .unwrap();
                let mut bytes = Vec::new();
                while let Some(chunk) = response.chunk().await.unwrap() {
                    bytes.extend_from_slice(&chunk);
                    assert!(
                        bytes.len() <= 10 * 1024 * 1024,
                        "Analyzer download exceeded budget"
                    );
                }
                bytes
            })
    };
    assert_eq!(qualitygate::snapshot::digest(&bytes), DIGEST);
    let path = directory.join("japicmp-0.26.2.jar");
    std::fs::write(&path, bytes).unwrap();
    path
}

fn run(root: &Path, code: i32) -> Value {
    report(&cli(root, &["check", "--format", "json"]), code)
}

#[test]
#[ignore = "requires JDK tools and verified japicmp distribution; mandatory Java compatibility CI job"]
fn real_java_binary_source_and_classpath_compatibility_repair() {
    let root = fixture();
    let external = tempfile::tempdir().unwrap();
    let javac = std::env::var("QUALITYGATE_TEST_JAVAC").unwrap_or_else(|_| "javac".into());
    let jar = std::env::var("QUALITYGATE_TEST_JAR").unwrap_or_else(|_| "jar".into());
    let source = external.path().join("builder.rs");
    let builder = external.path().join(if cfg!(windows) {
        "builder.exe"
    } else {
        "builder"
    });
    std::fs::write(&source, BUILDER).unwrap();
    let compiled = Command::new("rustc")
        .arg(source)
        .arg("-o")
        .arg(&builder)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let analyzer = analyzer(external.path());
    let mut policy = json!({"schema_version":1,"checks":[{
        "id":"api","argv":[builder,javac,jar],"timeout_seconds":60,
        "tools":[{"id":"javac","argv":[javac,"-version"]},{"id":"jar","argv":[jar,"--version"]}],
        "compatibility":{"tool":"japicmp","analyzer_jar":analyzer,"analyzer_sha256":DIGEST,
            "artifacts":[{"baseline":"target/api.jar","current":"target/api.jar"}],
            "classpath":[{"baseline":"target/dependency.jar","current":"target/dependency.jar"}],
            "level":"both","timeout_seconds":30}
    }]});
    let configure = |policy: &Value| {
        std::fs::write(
            root.path().join("qualitygate.yaml"),
            serde_norway::to_string(policy).unwrap(),
        )
        .unwrap()
    };
    configure(&policy);
    std::fs::write(root.path().join(".gitignore"), "target/\n").unwrap();
    std::fs::write(root.path().join("Api.java"), API).unwrap();
    std::fs::write(
        root.path().join("Base.java"),
        "public class Base { public String inherited() { return \"base\"; } }\n",
    )
    .unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-qm", "baseline public API"]);
    let original = run(root.path(), 0);
    assert_eq!(
        original["checks"][0]["metadata"]["compatibility"]["tool_version"],
        "0.26.2"
    );
    let broken = API.replace("public String greet(String value) { return value; } ", "");
    std::fs::write(root.path().join("Api.java"), broken).unwrap();
    let failed = run(root.path(), 1);
    assert!(
        failed["checks"][0]["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| diagnostic["evidence"]["symbol"] == "Api#greet(java.lang.String)")
    );
    std::fs::write(root.path().join("Api.java"), API).unwrap();
    run(root.path(), 0);
    // Explicit public construction remains available when adding a private overload.
    // japicmp 0.26.1 falsely reported this as CLASS_NOW_NOT_EXTENDABLE.
    std::fs::write(
        root.path().join("Api.java"),
        API.replace(
            "extends Base {",
            "extends Base { public Api() {} private Api(int value) {} ",
        ),
    )
    .unwrap();
    run(root.path(), 0);
    std::fs::write(
        root.path().join("Api.java"),
        API.replace("List<String>", "List<Integer>"),
    )
    .unwrap();
    let source_break = run(root.path(), 1);
    assert_eq!(
        source_break["checks"][0]["metadata"]["compatibility"]["comparison"]["binary_compatible"],
        true
    );
    assert_eq!(
        source_break["checks"][0]["metadata"]["compatibility"]["comparison"]["source_compatible"],
        false
    );
    policy["checks"][0]["compatibility"]["level"] = json!("binary");
    configure(&policy);
    run(root.path(), 0);
    policy["checks"][0]["compatibility"]["classpath"] = json!([]);
    configure(&policy);
    let missing = run(root.path(), 2);
    assert_eq!(missing["checks"][0]["verdict"], Value::Null);
    assert!(
        missing["checks"][0]["execution"]["artifacts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|artifact| artifact["path"].as_str().unwrap().ends_with("stderr.log"))
    );
    policy["checks"][0]["compatibility"]["artifacts"]
        .as_array_mut()
        .unwrap()
        .push(json!({"baseline":"target/dependency.jar", "current":"target/dependency.jar"}));
    configure(&policy);
    let multiple = run(root.path(), 0);
    assert_eq!(
        multiple["checks"][0]["metadata"]["compatibility"]["comparison"]["classes_compared"],
        2
    );
    std::fs::write(root.path().join("Base.java"), "public class Base {}\n").unwrap();
    let broken_dependency = run(root.path(), 1);
    assert!(
        broken_dependency["checks"][0]["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| diagnostic["evidence"]["symbol"] == "Base#inherited()")
    );
    git(root.path(), &["restore", "Base.java"]);
    run(root.path(), 0);
}
