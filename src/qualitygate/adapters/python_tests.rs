use super::*;
use crate::{
    config::{CustomRule, RuleSetting},
    snapshot::{File, Identity},
};
use serde_json::{Value, json};

const MANIFEST: &str = "[project]\nname='sample'\nversion='1.0'\nrequires-python='>=3.10'\ndependencies=['pytest>=8', 'colorama; sys_platform == \"win32\"']\n[project.optional-dependencies]\ntest=['helper>=1']\n";

fn fixture(root: &Path) -> (Snapshot, PythonProject, Value, BTreeMap<String, String>) {
    let metadata = |name: &str, version: &str, requirements: Value| json!({"name":name,"version":version,"requires_dist":requirements,"requires_python":null,"provides_extra":[]});
    let item = |metadata: Value| json!({"download_info":{"url":"https://example.invalid/package.whl"},"is_direct":false,"requested":false,"metadata":metadata});
    let mut project = item(metadata(
        "sample",
        "1.0",
        json!([
            "pytest>=8",
            "colorama; sys_platform == 'win32'",
            "helper>=1; extra == 'test'"
        ]),
    ));
    project["requested"] = json!(true);
    project["is_direct"] = json!(true);
    project["download_info"] = json!({"url":reqwest::Url::from_directory_path(root).unwrap().as_str().trim_end_matches('/'),"dir_info":{}});
    project["requested_extras"] = json!(["test"]);
    project["metadata"]["provides_extra"] = json!(["test"]);
    let report = json!({"version":"1","pip_version":"26.0.1","environment":{"implementation_name":"cpython","implementation_version":"3.12.3","os_name":"posix","platform_machine":"x86_64","platform_python_implementation":"CPython","platform_release":"6.0","platform_system":"Linux","platform_version":"fixture","python_full_version":"3.12.3","python_version":"3.12","sys_platform":"linux"},
        "install":[project,item(metadata("PyTest","8.4.2",json!(["pluggy>=1.5"]))),item(metadata("pluggy","1.6.0",json!([]))),item(metadata("helper","1.0",json!([])))]});
    let snapshot = Snapshot {
        root: root.into(),
        identity: Identity {
            mode: "worktree".into(),
            base: "base".into(),
            head: "head".into(),
            content_digest: "digest".into(),
            merge_request: None,
        },
        files: [
            (
                "pyproject.toml".into(),
                File {
                    bytes: MANIFEST.as_bytes().to_vec(),
                    executable: false,
                },
            ),
            (
                "tests/test_example.py".into(),
                File {
                    bytes: b"# Generated author=fixture\ndef test_example():\n    pass\n".to_vec(),
                    executable: false,
                },
            ),
        ]
        .into(),
        base_files: Default::default(),
        changes: Default::default(),
        path_filter: None,
        commits: vec![],
    };
    let spec = serde_json::from_value(json!({"ecosystem":"python","root":".","source_root":"src","test_source_root":"tests","install_target":"target/python","install_report":"target/pip.json","extras":["test"]})).unwrap();
    let versions = [
        ("python".into(), "Python 3.12.3".into()),
        (
            "pip".into(),
            "pip 26.0.1 from /tools/pip (python 3.12)".into(),
        ),
    ]
    .into();
    (snapshot, spec, report, versions)
}

fn installed(report: &Value) -> Vec<(String, Vec<u8>)> {
    report["install"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let metadata = &item["metadata"];
            let mut text = format!(
                "Metadata-Version: 2.4\nName: {}\nVersion: {}\n",
                metadata["name"].as_str().unwrap(),
                metadata["version"].as_str().unwrap()
            );
            for (key, field) in [
                ("requires_dist", "Requires-Dist"),
                ("provides_extra", "Provides-Extra"),
            ] {
                if let Some(values) = metadata[key].as_array() {
                    for value in values {
                        text.push_str(&format!("{field}: {}\n", value.as_str().unwrap()));
                    }
                }
            }
            if let Some(value) = metadata["requires_python"].as_str() {
                text.push_str(&format!("Requires-Python: {value}\n"));
            }
            text.push('\n');
            (format!("{index}.dist-info/METADATA"), text.into_bytes())
        })
        .collect()
}

#[test]
fn python_declarations_markers_extras_and_transitive_resolution_are_distinct() {
    let root = tempfile::tempdir().unwrap();
    let (snapshot, spec, report, versions) = fixture(root.path());
    let facts = parse(
        &spec,
        &serde_json::to_vec(&report).unwrap(),
        &installed(&report),
        root.path(),
        &snapshot,
        "facts",
        &versions,
    )
    .unwrap();
    assert_eq!(facts.ecosystem, "python");
    assert!(facts.has_test_dependency("pypi", "pytest"));
    assert!(facts.has_test_dependency("pypi", "helper"));
    assert!(!facts.has_test_dependency("pypi", "pluggy"));
    assert!(!facts.has_test_dependency("pypi", "colorama"));
    assert_eq!(facts.python.as_ref().unwrap().installed_metadata.len(), 4);
    let rule:CustomRule = serde_norway::from_str("id: paired\nsource: {document: AGENTS.md, section: Rules, content_hash: sha256:dummy}\nversion: 1\nlanguage: [python]\nrequires_capabilities: [test_methods,comments,dependency_resolution]\napplies_to: {paths: ['tests/**/*.py'], provenance_scope: all_added_tests}\nbinding: {marker: {type: comment,name: Generated,fields: [author]}}\nwhen: {entity: test_method,change: added}\nthen: {require_marker: true,require_dependency: {artifact: PYTEST}}\nfix: Restore the declared distribution\n").unwrap();
    let run = |facts: &[ProjectFacts]| {
        crate::adapters::custom_rules::evaluate_with_projects(
            &rule,
            &RuleSetting::default(),
            &snapshot,
            facts,
        )
    };
    assert_eq!(
        run(std::slice::from_ref(&facts)).verdict,
        Some(Verdict::Pass)
    );
    let mut removed = facts.clone();
    removed.declared.clear();
    assert_eq!(run(&[removed]).verdict, Some(Verdict::Fail));
    let mut wrong = facts.clone();
    wrong.test_source_root = "other/tests".into();
    assert_eq!(run(&[wrong]).execution.status, ExecutionStatus::Blocked);
    let mut java = facts.clone();
    java.ecosystem = "maven".into();
    java.python = None;
    assert_eq!(run(&[java, facts]).verdict, Some(Verdict::Pass));
}

#[test]
fn python_reports_must_match_manifest_installation_versions_and_closure() {
    let root = tempfile::tempdir().unwrap();
    let (snapshot, spec, original, versions) = fixture(root.path());
    let check = |report: &Value, snapshot: &Snapshot, metadata: &[(String, Vec<u8>)]| {
        parse(
            &spec,
            &serde_json::to_vec(report).unwrap(),
            metadata,
            root.path(),
            snapshot,
            "facts",
            &versions,
        )
    };
    for (pointer, value) in [
        ("/version", json!("2")),
        ("/pip_version", json!("1")),
        ("/environment/python_full_version", json!("3.13.1")),
        ("/install/0/requested", json!(false)),
        ("/install/0/is_direct", json!(false)),
        ("/install/0/requested_extras", json!([])),
        (
            "/install/0/download_info/url",
            json!("file:///other-project"),
        ),
        (
            "/install/0/download_info/dir_info",
            json!({"editable":true}),
        ),
        ("/install/0/metadata/version", json!("2")),
        ("/install/0/metadata/requires_dist", json!([])),
        ("/install/1/metadata/version", json!("7.0")),
        ("/install/1/metadata/requires_python", json!(">=4")),
        ("/install/1/requested", json!(true)),
        ("/install/2/metadata/name", json!("PYTEST")),
        ("/install/0/metadata/provides_extra", json!([])),
        ("/install/1/metadata/requires_dist", json!(["missing>=1"])),
        (
            "/install/1/metadata/requires_dist",
            json!(["pluggy[unknown]>=1.5"]),
        ),
        ("/install/1/metadata/requires_dist", json!(["pluggy>=2"])),
        (
            "/install/1/metadata/requires_dist",
            json!(["pluggy @ https://example.invalid/pluggy.whl"]),
        ),
    ] {
        let mut report = original.clone();
        *report
            .pointer_mut(pointer)
            .unwrap_or_else(|| panic!("{pointer}")) = value;
        assert!(
            check(&report, &snapshot, &installed(&report)).is_err(),
            "{pointer}"
        );
    }
    let mut missing = original.clone();
    missing["install"].as_array_mut().unwrap().remove(2);
    assert!(check(&missing, &snapshot, &installed(&missing)).is_err());
    let mut extra = original.clone();
    let mut orphan = original["install"][2].clone();
    orphan["metadata"]["name"] = json!("orphan");
    extra["install"].as_array_mut().unwrap().push(orphan);
    assert!(check(&extra, &snapshot, &installed(&extra)).is_err());
    let mut changed = snapshot.clone();
    changed.files.get_mut("pyproject.toml").unwrap().bytes =
        MANIFEST.replace("pytest>=8", "pytest>=9").into_bytes();
    assert!(check(&original, &changed, &installed(&original)).is_err());
    for text in [
        "[project]\nname='sample'\ndynamic=['version']",
        "[project]\nname='sample'\nversion='1'\ndependencies=12",
        "[project]\nname='sample'\nversion='1'\nrequires-python='>=4'",
        "[tool.poetry]\nname='sample'",
    ] {
        let mut changed = snapshot.clone();
        changed.files.get_mut("pyproject.toml").unwrap().bytes = text.as_bytes().to_vec();
        assert!(check(&original, &changed, &installed(&original)).is_err());
    }
    assert!(check(&original, &snapshot, &[]).is_err());
    let mut metadata = installed(&original);
    metadata[1].1 = String::from_utf8(metadata[1].1.clone())
        .unwrap()
        .replace("Version: 8.4.2", "Version: 8.0")
        .into_bytes();
    assert!(check(&original, &snapshot, &metadata).is_err());
    let mut duplicated = installed(&original);
    duplicated.push(duplicated[0].clone());
    assert!(check(&original, &snapshot, &duplicated).is_err());
}

#[test]
fn metadata_headers_and_requirement_budgets_reject_ambiguous_input() {
    assert!(
        python_metadata::parse(
            b"Metadata-Version: 2.4\nName: pkg\nVersion: 1\nRequires-Dist: other\n >=1\n\nbody"
        )
        .is_ok()
    );
    for text in [
        " continuation",
        "malformed",
        "Metadata-Version: 99\nName: pkg\nVersion: 1",
        "Metadata-Version: 2.4\nName: pkg\nName: duplicate\nVersion: 1",
        "Metadata-Version: 2.4\nVersion: 1",
    ] {
        assert!(python_metadata::parse(text.as_bytes()).is_err());
    }
    assert!(all_requirements(&["x".repeat(8193)]).is_err());
    assert!(
        all_requirements(&[format!(
            "x; {}python_version>'3'{}",
            "(".repeat(65),
            ")".repeat(65)
        )])
        .is_err()
    );
}

#[test]
fn extras_revisit_dependencies_and_keep_their_marker_context() {
    let root = tempfile::tempdir().unwrap();
    let (mut snapshot, spec, mut report, versions) = fixture(root.path());
    snapshot.files.get_mut("pyproject.toml").unwrap().bytes = MANIFEST
        .replace("'helper>=1'", "'helper>=1; extra == \"test\"'")
        .into_bytes();
    report["install"][3]["metadata"]["requires_dist"] = json!(["pluggy[feature]>=1.5"]);
    report["install"][2]["metadata"]["provides_extra"] = json!(["feature"]);
    report["install"][2]["metadata"]["requires_dist"] = json!(["conditional; extra == 'feature'"]);
    let mut item = report["install"][3].clone();
    item["metadata"]["name"] = json!("conditional");
    item["metadata"]["requires_dist"] = json!([]);
    report["install"].as_array_mut().unwrap().push(item);
    let facts = parse(
        &spec,
        &serde_json::to_vec(&report).unwrap(),
        &installed(&report),
        root.path(),
        &snapshot,
        "facts",
        &versions,
    )
    .unwrap();
    assert!(
        facts
            .resolved
            .iter()
            .any(|dependency| dependency.artifact == "conditional")
    );
    assert!(!facts.has_test_dependency("pypi", "conditional"));
    report["environment"]["python_version"] = json!("3.9");
    assert!(
        parse(
            &spec,
            &serde_json::to_vec(&report).unwrap(),
            &installed(&report),
            root.path(),
            &snapshot,
            "facts",
            &versions
        )
        .is_err()
    );
}
