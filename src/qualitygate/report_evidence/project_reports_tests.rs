use super::*;
use crate::domain::Severity;
use serde_json::json;

#[tokio::test]
async fn maven_collection_binds_fresh_model_tree_and_producer_and_retains_bad_raw_reports() {
    let root = tempfile::tempdir().unwrap();
    let artifacts = tempfile::tempdir().unwrap();
    let mut snapshot = crate::application::test_support::snapshot(root.path());
    snapshot.files.insert(
        "pom.xml".into(),
        crate::snapshot::File {
            bytes: b"<project><artifactId>sample</artifactId></project>".to_vec(),
            executable: false,
        },
    );
    let snapshot = Arc::new(snapshot);
    let mut check:CommandCheck=serde_json::from_value(json!({"id":"model","argv":["mvn"],"projects":[{"root":".","effective_pom":"effective.xml","dependency_tree":"tree.json"}]})).unwrap();
    let model = format!(
        "<project><modelVersion>4.0.0</modelVersion><groupId>unit</groupId><artifactId>sample</artifactId><version>1</version><build><sourceDirectory>{}/src/main/java</sourceDirectory><testSourceDirectory>{}/src/test/java</testSourceDirectory></build></project>",
        root.path().display(),
        root.path().display()
    );
    std::fs::write(root.path().join("effective.xml"), &model).unwrap();
    std::fs::write(root.path().join("tree.json"),json!({"groupId":"unit","artifactId":"sample","version":"1","type":"jar","classifier":"","optional":"false","scope":"","children":[]}).to_string()).unwrap();
    let mut result = CheckResult::pending("model", true, Severity::Error);
    collect(
        &check,
        root.path(),
        artifacts.path(),
        &snapshot,
        b"",
        &mut result,
    )
    .await
    .unwrap();
    assert_eq!(result.execution.artifacts.len(), 2);
    assert_eq!(result.metadata["projects"][0]["producer_check"], "model");
    assert_eq!(
        result.metadata["projects"][0]["snapshot_digest"],
        snapshot.identity.content_digest
    );
    let ProjectSpec::Maven(project) = &mut check.projects[0] else {
        panic!("Maven")
    };
    project.dependency_usage = true;
    let mut usage = CheckResult::pending("model", true, Severity::Error);
    assert!(
        collect(
            &check,
            root.path(),
            artifacts.path(),
            &snapshot,
            b"missing dependency analysis",
            &mut usage
        )
        .await
        .is_err()
    );
    assert_eq!(usage.execution.artifacts.len(), 2);
    std::fs::write(root.path().join("effective.xml"), b"invalid XML").unwrap();
    let mut invalid = CheckResult::pending("model", true, Severity::Error);
    assert!(
        collect(
            &check,
            root.path(),
            artifacts.path(),
            &snapshot,
            b"",
            &mut invalid
        )
        .await
        .is_err()
    );
    assert_eq!(
        std::fs::read(&invalid.execution.artifacts[0].path).unwrap(),
        b"invalid XML"
    );
    assert!(!invalid.metadata.contains_key("projects"));
}

#[tokio::test]
async fn python_installation_matches_distribution_metadata_and_requires_tool_versions() {
    let root = tempfile::tempdir().unwrap();
    let artifacts = tempfile::tempdir().unwrap();
    let mut snapshot = crate::application::test_support::snapshot(root.path());
    snapshot.files.insert(
        "pyproject.toml".into(),
        crate::snapshot::File {
            bytes: b"[project]\nname='sample'\nversion='1.0'\ndependencies=[]\n".to_vec(),
            executable: false,
        },
    );
    let snapshot = Arc::new(snapshot);
    let check:CommandCheck=serde_json::from_value(json!({"id":"python","argv":["python"],"projects":[{"ecosystem":"python","root":".","source_root":"src","test_source_root":"tests","install_target":"installed","install_report":"pip.json"}]})).unwrap();
    let report = json!({"version":"1","pip_version":"26.0.1","environment":{"implementation_name":"cpython","implementation_version":"3.12.3","os_name":"posix","platform_machine":"x86_64","platform_python_implementation":"CPython","platform_release":"6.0","platform_system":"Linux","platform_version":"fixture","python_full_version":"3.12.3","python_version":"3.12","sys_platform":"linux"},
        "install":[{"download_info":{"url":reqwest::Url::from_directory_path(root.path()).unwrap().as_str().trim_end_matches('/'),"dir_info":{}},"is_direct":true,"requested":true,
        "metadata":{"name":"sample","version":"1.0","requires_dist":[],"requires_python":null,"provides_extra":[]}}]});
    std::fs::write(
        root.path().join("pip.json"),
        serde_json::to_vec(&report).unwrap(),
    )
    .unwrap();
    std::fs::create_dir_all(root.path().join("installed/sample.dist-info")).unwrap();
    std::fs::write(
        root.path().join("installed/sample.dist-info/METADATA"),
        b"Metadata-Version: 2.4\nName: sample\nVersion: 1.0\n\n",
    )
    .unwrap();
    let mut result = CheckResult::pending("python", true, Severity::Error);
    assert!(
        collect(
            &check,
            root.path(),
            artifacts.path(),
            &snapshot,
            b"",
            &mut result
        )
        .await
        .unwrap_err()
        .to_string()
        .contains("tool versions")
    );
    assert_eq!(result.execution.artifacts.len(), 2);
    result.metadata.insert("tools".into(),json!([{"id":"python","version":"Python 3.12.3"},{"id":"pip","version":"pip 26.0.1 from /tools/pip (python 3.12)"}]));
    collect(
        &check,
        root.path(),
        artifacts.path(),
        &snapshot,
        b"",
        &mut result,
    )
    .await
    .unwrap();
    assert_eq!(result.metadata["projects"][0]["ecosystem"], "python");
    assert_eq!(result.metadata["projects"][0]["producer_check"], "python");
    let mut invalid = result.clone();
    invalid
        .metadata
        .insert("tools".into(), json!([{"version":"missing id"}]));
    assert!(
        collect(
            &check,
            root.path(),
            artifacts.path(),
            &snapshot,
            b"",
            &mut invalid
        )
        .await
        .is_err()
    );
    invalid
        .metadata
        .insert("tools".into(), json!([{"id":"python"}]));
    assert!(
        collect(
            &check,
            root.path(),
            artifacts.path(),
            &snapshot,
            b"",
            &mut invalid
        )
        .await
        .is_err()
    );
}
