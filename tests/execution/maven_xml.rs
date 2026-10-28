use super::*;
use std::process::Command;

#[test]
fn bounded_maven_xml_retains_reports_and_blocks_dependent_checks() {
    let root = fixture();
    let root = root.path();
    let external = tempfile::tempdir().unwrap();
    let source = external.path().join("producer.rs");
    let binary = external.path().join("producer.exe");
    std::fs::write(
        &source,
        r#"
fn main() {
    if std::env::args().any(|arg| arg == "--version") {
        println!("maven model fixture 1");
        return;
    }
    let workspace = std::env::current_dir().unwrap().display().to_string()
        .replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let model = std::fs::read_to_string("model.input").unwrap().replace("@WORKSPACE@", &workspace);
    std::fs::create_dir_all("target").unwrap();
    std::fs::write("target/effective.xml", &model).unwrap();
    std::fs::copy("tree.input", "target/tree.json").unwrap();
    print!("{model}");
}
"#,
    )
    .unwrap();
    let built = Command::new("rustc")
        .arg(source)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    std::fs::write(root.join(".gitignore"), "target/\n").unwrap();
    let manifest = "<project><artifactId>sample</artifactId></project>";
    let model = "<project><modelVersion>4.0.0</modelVersion><groupId>fixture</groupId><artifactId>sample</artifactId><version>1</version><build><sourceDirectory>@WORKSPACE@/src/main/java</sourceDirectory><testSourceDirectory>@WORKSPACE@/src/test/java</testSourceDirectory></build></project>";
    let tree = serde_json::to_vec(&json!({"groupId":"fixture","artifactId":"sample","version":"1","type":"jar","classifier":"","scope":"","optional":"false","children":[]})).unwrap();
    std::fs::write(root.join("tree.input"), &tree).unwrap();
    policy(
        root,
        json!([
            {"id":"maven","argv":[binary],"tools":[{"id":"producer","argv":[binary,"--version"]}],
             "projects":[{"root":".","effective_pom":"target/effective.xml","dependency_tree":"target/tree.json"}]},
            {"id":"dependent","argv":["git","--version"],"depends_on":["maven"]}
        ]),
    );
    for (padding, reason) in [
        (
            format!("{}{}", "<nested>".repeat(2_000), "</nested>".repeat(2_000)),
            "nesting exceeds",
        ),
        ("<n/>".repeat(100_000), "nodes limit reached"),
    ] {
        for bad_manifest in [true, false] {
            let padded = |xml: &str| xml.replace("</project>", &format!("{padding}</project>"));
            std::fs::write(
                root.join("pom.xml"),
                if bad_manifest {
                    padded(manifest)
                } else {
                    manifest.into()
                },
            )
            .unwrap();
            std::fs::write(
                root.join("model.input"),
                if bad_manifest {
                    model.into()
                } else {
                    padded(model)
                },
            )
            .unwrap();
            let value = report(&cli(root, &["check", "--format", "json"]), 2);
            assert_eq!(value["gate"]["complete"], false);
            let check = &value["checks"][0];
            assert_eq!(check["execution"]["status"], "tool_error");
            assert!(
                check["execution"]["reason"]
                    .as_str()
                    .unwrap()
                    .contains(reason)
            );
            assert!(check["verdict"].is_null());
            assert!(check["metadata"]["projects"].is_null());
            assert!(check["diagnostics"].as_array().unwrap().is_empty());
            let retained = |suffix: &str| {
                let artifact = check["execution"]["artifacts"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|item| item["path"].as_str().unwrap().ends_with(suffix))
                    .unwrap();
                let bytes = std::fs::read(artifact["path"].as_str().unwrap()).unwrap();
                assert_eq!(artifact["digest"], qualitygate::snapshot::digest(&bytes));
                assert_eq!(artifact["bytes"], bytes.len());
                bytes
            };
            let actual_model = retained("project-0-model");
            assert_eq!(actual_model, retained("-stdout.log"));
            if !bad_manifest {
                assert!(String::from_utf8(actual_model).unwrap().contains(&padding));
            }
            assert_eq!(retained("project-0-tree"), tree);
            assert_eq!(value["checks"][1]["execution"]["status"], "blocked");
            assert!(value["checks"][1]["execution"]["started_at_ms"].is_null());
        }
    }
    std::fs::write(root.join("pom.xml"), manifest).unwrap();
    std::fs::write(root.join("model.input"), model).unwrap();
    let value = report(&cli(root, &["check", "--format", "json"]), 0);
    assert_eq!(value["gate"]["complete"], true);
    assert_eq!(
        value["checks"][0]["metadata"]["projects"][0]["coordinate"],
        "fixture:sample:1"
    );
    assert_eq!(value["checks"][1]["verdict"], "pass");
}
