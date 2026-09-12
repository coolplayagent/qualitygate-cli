mod common;
use common::*;
use serde_json::{Value, json};
use std::{
    io::{Cursor, Write},
    path::Path,
    process::Command,
};
use zip::{ZipWriter, write::SimpleFileOptions};

fn jar(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut archive = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in entries {
        archive
            .start_file(
                *name,
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
            )
            .unwrap();
        archive.write_all(bytes).unwrap();
    }
    archive.finish().unwrap().into_inner()
}

const DRIVER: &str = r#"
use std::{fs,process};
fn escape(value:&str)->String { value.replace('&',"&amp;").replace('<',"&lt;").replace('>',"&gt;").replace('"',"&quot;") }
fn main() {
    let args:Vec<_> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--version" || arg == "-version") { println!("fixture-tool 1"); return; }
    if args[1] == "build" {
        let mode = fs::read_to_string("control.txt").unwrap();
        if mode.contains("build-fail") { process::exit(7); }
        if mode.contains("build-timeout") { std::thread::sleep(std::time::Duration::from_secs(3)); }
        if mode.contains("missing-output") { return; }
        fs::create_dir_all("target").unwrap();
        fs::copy("api.bin","target/api.jar").unwrap();
        if fs::exists("dependency.bin").unwrap() { fs::copy("dependency.bin","target/dependency.jar").unwrap(); }
        if mode.contains("mutate-source") { fs::write("control.txt","changed").unwrap(); }
        return;
    }
    let arg = |name| args.iter().position(|arg| arg == name).map(|index| args[index+1].as_str()).unwrap();
    let old = arg("--old");
    let new = arg("--new");
    let bytes = fs::read(new).unwrap();
    let contains = |mode:&str| bytes.windows(mode.len()).any(|window| window == mode.as_bytes());
    if contains("missing-report") { return; }
    if contains("timeout") { std::thread::sleep(std::time::Duration::from_secs(3)); }
    if contains("crash") { process::exit(1); }
    if contains("mutate-archive") { let mut bytes=fs::read(old).unwrap(); bytes.push(b' '); fs::write(old,bytes).unwrap(); }
    let binary = !contains("breaking");
    let source = binary && !contains("source-only");
    let old = escape(if contains("foreign") { "/different/old.jar" } else { old });
    let new = escape(new);
    let classes = if contains("partial") { String::new() } else {
        let changes = if binary && source { String::new() } else { format!(
            "<methods><method name=\"greet\" binaryCompatible=\"{binary}\" sourceCompatible=\"{source}\"><parameters><parameter type=\"java.lang.String\"/></parameters><compatibilityChanges><compatibilityChange type=\"METHOD_REMOVED\" binaryCompatible=\"{binary}\" sourceCompatible=\"{source}\"/></compatibilityChanges></method></methods>"
        )};
        format!("<class fullyQualifiedName=\"Api\" binaryCompatible=\"{binary}\" sourceCompatible=\"{source}\">{changes}</class>")
    };
    fs::write(arg("--xml-file"),format!(
        "<japicmp oldJar=\"{old}\" newJar=\"{new}\" accessModifier=\"PRIVATE\" packagesInclude=\"all\" packagesExclude=\"n.a.\" ignoreMissingClasses=\"false\" ignoreMissingClassesByRegularExpressions=\"\" onlyModifications=\"false\" onlyBinaryIncompatibleModifications=\"false\"><classes>{classes}</classes></japicmp>"
    )).unwrap();
}
"#;

struct Fixture {
    root: tempfile::TempDir,
    _tools: tempfile::TempDir,
    policy: Value,
}

impl Fixture {
    fn new() -> Self {
        let root = fixture();
        let tools = tempfile::tempdir().unwrap();
        let driver_source = tools.path().join("driver.rs");
        let driver = tools.path().join(if cfg!(windows) {
            "driver.exe"
        } else {
            "driver"
        });
        std::fs::write(&driver_source, DRIVER).unwrap();
        let compiled = Command::new("rustc")
            .arg(&driver_source)
            .arg("-o")
            .arg(&driver)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let analyzer = jar(&[(
            "META-INF/maven/com.github.siom79.japicmp/japicmp/pom.properties",
            b"groupId=com.github.siom79.japicmp\nartifactId=japicmp\nversion=0.26.2\n",
        )]);
        let analyzer_path = tools.path().join("analyzer.jar");
        std::fs::write(&analyzer_path, &analyzer).unwrap();
        let policy = json!({"schema_version":1,"checks":[{
            "id":"api","argv":[driver,"build"],"tools":[{"id":"builder","argv":[driver,"--version"]}],
            "compatibility":{"tool":"japicmp","java":driver,"analyzer_jar":analyzer_path,
                "analyzer_sha256":qualitygate::snapshot::digest(&analyzer),
                "artifacts":[{"baseline":"target/api.jar","current":"target/api.jar"}],"timeout_seconds":1}
        }]});
        std::fs::write(root.path().join("control.txt"), "ok").unwrap();
        std::fs::write(root.path().join(".gitignore"), "target/\n").unwrap();
        let fixture = Self {
            root,
            _tools: tools,
            policy,
        };
        fixture.set_mode("ok");
        fixture.configure(&fixture.policy);
        git(fixture.root.path(), &["add", "."]);
        git(
            fixture.root.path(),
            &["commit", "-qm", "baseline API and policy"],
        );
        fixture
    }

    fn configure(&self, policy: &Value) {
        std::fs::write(
            self.root.path().join("qualitygate.yaml"),
            serde_norway::to_string(policy).unwrap(),
        )
        .unwrap();
    }
    fn set_mode(&self, mode: &str) {
        std::fs::write(
            self.root.path().join("api.bin"),
            jar(&[
                ("Api.class", b"\xca\xfe\xba\xbe"),
                ("fixture-mode.txt", mode.as_bytes()),
            ]),
        )
        .unwrap();
    }
    fn run(&self, extra: &[&str], code: i32) -> Value {
        let mut args = vec!["check", "--format", "json"];
        args.extend_from_slice(extra);
        report(&cli(self.root.path(), &args), code)
    }
}

#[test]
fn paired_builds_bind_artifacts_find_breaks_and_pass_after_repair() {
    let fixture = Fixture::new();
    fixture.set_mode("breaking");
    let failed = fixture.run(&[], 1);
    let check = &failed["checks"][0];
    assert_eq!(check["execution"]["status"], "completed");
    assert_eq!(
        check["diagnostics"][0]["evidence"]["symbol"],
        "Api#greet(java.lang.String)"
    );
    assert_eq!(
        check["metadata"]["compatibility"]["comparison"]["classes_compared"],
        1
    );
    assert_eq!(
        check["metadata"]["baseline_build"]["metadata"]["execution_snapshot"]["head"],
        failed["snapshot"]["base"]
    );
    assert_eq!(
        check["metadata"]["current_build"]["metadata"]["execution_snapshot"],
        failed["snapshot"]
    );
    let archives = check["metadata"]["compatibility"]["archives"]
        .as_array()
        .unwrap();
    assert_eq!(archives.len(), 2);
    assert_ne!(
        archives[0]["artifact"]["digest"],
        archives[1]["artifact"]["digest"]
    );
    for archive in archives {
        assert!(Path::new(archive["artifact"]["path"].as_str().unwrap()).is_file());
    }
    fixture.set_mode("ok");
    let repaired = fixture.run(&[], 0);
    assert_eq!(
        repaired["checks"][0]["metadata"]["compatibility"]["comparison"]["binary_compatible"],
        true
    );
    assert_eq!(
        std::fs::read_to_string(fixture.root.path().join("control.txt")).unwrap(),
        "ok"
    );
}

#[test]
fn incomplete_analyzer_output_timeout_and_input_mutation_never_pass() {
    let fixture = Fixture::new();
    for mode in [
        "missing-report",
        "foreign",
        "partial",
        "crash",
        "mutate-archive",
        "timeout",
    ] {
        fixture.set_mode(mode);
        let output = fixture.run(&[], 2);
        assert_eq!(output["checks"][0]["verdict"], Value::Null, "{mode}");
        if mode == "timeout" {
            assert_eq!(output["checks"][0]["execution"]["status"], "timed_out");
        }
        assert!(
            !output["checks"][0]["execution"]["artifacts"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn missing_build_outputs_and_source_changes_block_comparison() {
    let fixture = Fixture::new();
    for mode in ["build-fail", "missing-output", "mutate-source"] {
        std::fs::write(fixture.root.path().join("control.txt"), mode).unwrap();
        let output = fixture.run(&[], 2);
        assert_eq!(output["checks"][0]["verdict"], Value::Null);
        assert_eq!(
            std::fs::read_to_string(fixture.root.path().join("control.txt")).unwrap(),
            mode
        );
    }
    git(fixture.root.path(), &["add", "control.txt"]);
    git(
        fixture.root.path(),
        &["commit", "-qm", "invalid baseline build"],
    );
    std::fs::write(fixture.root.path().join("control.txt"), "ok").unwrap();
    let output = fixture.run(&[], 2);
    assert!(
        output["checks"][0]["metadata"]
            .get("current_build")
            .is_none()
    );
}

#[test]
fn comparison_uses_staged_inputs_and_respects_binary_source_policy() {
    let fixture = Fixture::new();
    fixture.set_mode("breaking");
    git(fixture.root.path(), &["add", "api.bin"]);
    fixture.set_mode("ok");
    fixture.run(&[], 0);
    fixture.run(&["--staged"], 1);
    fixture.set_mode("source-only");
    fixture.run(&[], 1);
    let mut policy = fixture.policy.clone();
    policy["checks"][0]["compatibility"]["level"] = json!("binary");
    fixture.configure(&policy);
    fixture.run(&["--policy-ref", "HEAD"], 2);
    let binary = fixture.run(&[], 0);
    assert_eq!(
        binary["checks"][0]["metadata"]["compatibility"]["comparison"]["source_compatible"],
        false
    );
    policy["checks"][0]["compatibility"]["level"] = json!("source");
    fixture.configure(&policy);
    fixture.run(&[], 1);
}

#[test]
fn internal_build_evidence_cannot_collide_with_named_checks() {
    let fixture = Fixture::new();
    let mut policy = fixture.policy.clone();
    let driver = policy["checks"][0]["argv"][0].clone();
    policy["checks"].as_array_mut().unwrap().push(json!({
        "id":"api.baseline-build", "argv":[driver,"--version"]
    }));
    fixture.configure(&policy);
    let output = fixture.run(&[], 0);
    let check = &output["checks"][0];
    for build in ["baseline_build", "current_build"] {
        for artifact in check["metadata"][build]["execution"]["artifacts"]
            .as_array()
            .unwrap()
        {
            let bytes = std::fs::read(artifact["path"].as_str().unwrap()).unwrap();
            assert_eq!(qualitygate::snapshot::digest(&bytes), artifact["digest"]);
        }
    }
    assert!(
        check["execution"]["artifacts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|artifact| artifact["path"]
                .as_str()
                .unwrap()
                .ends_with("compatibility-analyzer.jar"))
    );
}

#[test]
fn classpath_inventory_and_tool_inputs_remain_complete_and_comparable() {
    let fixture = Fixture::new();
    let mut policy = fixture.policy.clone();
    policy["checks"][0]["compatibility"]["classpath"] = json!([
        {"baseline":"target/dependency.jar", "current":"target/dependency.jar"}
    ]);
    fixture.configure(&policy);
    std::fs::write(
        fixture.root.path().join("dependency.bin"),
        jar(&[("Base.class", b"class")]),
    )
    .unwrap();
    git(fixture.root.path(), &["add", "."]);
    git(
        fixture.root.path(),
        &["commit", "-qm", "complete classpath"],
    );
    let output = fixture.run(&[], 0);
    assert_eq!(
        output["checks"][0]["metadata"]["compatibility"]["archives"]
            .as_array()
            .unwrap()
            .len(),
        4
    );
    std::fs::write(
        fixture.root.path().join("dependency.bin"),
        jar(&[("Api.class", b"duplicate")]),
    )
    .unwrap();
    fixture.run(&[], 2);
    git(fixture.root.path(), &["restore", "dependency.bin"]);
    policy["checks"][0]["tools"][0]["inputs"] = json!(["control.txt"]);
    fixture.configure(&policy);
    std::fs::write(
        fixture.root.path().join("control.txt"),
        "different tool input",
    )
    .unwrap();
    let output = fixture.run(&[], 2);
    assert!(
        output["checks"][0]["execution"]["reason"]
            .as_str()
            .unwrap()
            .contains("tool inputs differ")
    );
    policy["checks"][0]["tools"][0]
        .as_object_mut()
        .unwrap()
        .remove("inputs");
    policy["checks"][0]["timeout_seconds"] = json!(1);
    fixture.configure(&policy);
    std::fs::write(fixture.root.path().join("control.txt"), "build-timeout").unwrap();
    let output = fixture.run(&[], 2);
    assert_eq!(output["checks"][0]["execution"]["status"], "timed_out");
}

#[test]
fn analyzer_digest_archive_inventory_and_configuration_are_required() {
    let fixture = Fixture::new();
    let mut policy = fixture.policy.clone();
    policy["checks"][0]["compatibility"]["analyzer_sha256"] =
        json!(format!("sha256:{}", "0".repeat(64)));
    fixture.configure(&policy);
    let output = fixture.run(&[], 2);
    assert!(
        output["checks"][0]["metadata"]
            .get("baseline_build")
            .is_none()
    );
    fixture.configure(&fixture.policy);
    std::fs::write(fixture.root.path().join("api.bin"), b"not a JAR").unwrap();
    fixture.run(&[], 2);
    for (key, value) in [
        ("artifacts", json!([])),
        ("timeout_seconds", json!(0)),
        ("level", json!("unchecked")),
        (
            "classpath",
            json!([{"baseline":"../bad.jar","current":"target/cp.jar"}]),
        ),
    ] {
        let mut policy = fixture.policy.clone();
        policy["checks"][0]["compatibility"][key] = value;
        assert!(
            qualitygate::config::parse(serde_norway::to_string(&policy).unwrap().as_bytes())
                .is_err()
        );
    }
}

#[test]
fn task_contract_can_require_a_complete_compatibility_check() {
    let fixture = Fixture::new();
    fixture.configure(&json!({"schema_version":1}));
    let mut verification = fixture.policy["checks"][0].clone();
    verification.as_object_mut().unwrap().remove("id");
    verification["check_id"] = json!("api");
    let task = json!({"schema_version":1,"task_id":"preserve-api","acceptance":[{
        "id":"compatible","description":"Existing callers remain compatible","verification":verification
    }]});
    std::fs::write(
        fixture.root.path().join("task.yaml"),
        serde_norway::to_string(&task).unwrap(),
    )
    .unwrap();
    fixture.set_mode("breaking");
    let failed = fixture.run(&["--task", "task.yaml"], 1);
    assert_eq!(failed["plan"]["acceptance"]["compatible"], "api");
    fixture.set_mode("ok");
    fixture.run(&["--task", "task.yaml"], 0);
}
