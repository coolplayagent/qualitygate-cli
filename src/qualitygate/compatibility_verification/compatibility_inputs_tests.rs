use super::*;
use crate::domain::Severity;
use serde_json::json;
use std::io::{Cursor, Write};

fn archive(name: &str) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file(name, zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(b"inventory-only fixture").unwrap();
    zip.finish().unwrap().into_inner()
}

fn spec() -> CompatibilitySpec {
    serde_json::from_value(
        json!({"tool":"japicmp", "analyzer_jar":"analyzer.jar", "analyzer_sha256":"unused",
        "artifacts":[{"baseline":"old.jar", "current":"new.jar"}],
        "classpath":[{"baseline":"old-dependency.jar", "current":"new-dependency.jar"}]}),
    )
    .unwrap()
}

#[tokio::test]
async fn captured_api_and_classpath_remain_separate_and_bind_exact_snapshot_and_archive_bytes() {
    let workspace = tempfile::tempdir().unwrap();
    let artifacts = tempfile::tempdir().unwrap();
    let snapshot = crate::application::test_support::snapshot(workspace.path());
    let api = archive("api/Example.class");
    let dependency = archive("dependency/Helper.class");
    for name in ["old.jar", "new.jar"] {
        std::fs::write(workspace.path().join(name), &api).unwrap();
    }
    for name in ["old-dependency.jar", "new-dependency.jar"] {
        std::fs::write(workspace.path().join(name), &dependency).unwrap();
    }
    for baseline in [true, false] {
        let mut result = CheckResult::pending("compatibility", true, Severity::Error);
        let captured = collect(
            &spec(),
            baseline,
            workspace.path(),
            artifacts.path(),
            &snapshot,
            &mut result,
        )
        .await
        .unwrap();
        let side = if baseline { "baseline" } else { "current" };
        assert_eq!(captured.classes, BTreeSet::from(["api.Example".into()]));
        assert_eq!(captured.archives, [format!("{side}/api-0.jar")]);
        assert_eq!(captured.classpath, [format!("{side}/classpath-0.jar")]);
        assert_eq!(captured.files.len(), 2);
        assert_eq!(captured.bindings.len(), 2);
        for binding in captured.bindings {
            assert_eq!(
                binding.source_snapshot_digest,
                snapshot.identity.content_digest
            );
            let expected = if binding.role == "api" {
                &api
            } else {
                &dependency
            };
            assert_eq!(std::fs::read(binding.artifact.path).unwrap(), *expected);
            assert_eq!(binding.artifact.digest, crate::snapshot::digest(expected));
            assert_eq!(captured.files[&binding.input_path].bytes, *expected);
        }
        assert_eq!(result.execution.artifacts.len(), 2);
    }
}

#[tokio::test]
async fn duplicate_classes_or_malformed_archives_cannot_form_compatibility_inventory() {
    let workspace = tempfile::tempdir().unwrap();
    let artifacts = tempfile::tempdir().unwrap();
    let snapshot = crate::application::test_support::snapshot(workspace.path());
    let bytes = archive("api/Duplicate.class");
    for name in ["new.jar", "new-dependency.jar"] {
        std::fs::write(workspace.path().join(name), &bytes).unwrap();
    }
    let mut result = CheckResult::pending("compatibility", true, Severity::Error);
    let error = collect(
        &spec(),
        false,
        workspace.path(),
        artifacts.path(),
        &snapshot,
        &mut result,
    )
    .await
    .err()
    .unwrap();
    assert!(error.to_string().contains("Duplicate class"));
    assert_eq!(result.execution.artifacts.len(), 1);
    std::fs::write(workspace.path().join("new.jar"), b"not a jar").unwrap();
    assert!(
        collect(
            &spec(),
            false,
            workspace.path(),
            artifacts.path(),
            &snapshot,
            &mut result
        )
        .await
        .is_err()
    );
}

#[tokio::test]
async fn stale_outputs_can_be_removed_but_checked_inputs_and_unbounded_files_are_rejected() {
    let workspace = tempfile::tempdir().unwrap();
    let mut snapshot = crate::application::test_support::snapshot(workspace.path());
    std::fs::write(workspace.path().join("new.jar"), b"checked").unwrap();
    snapshot.files.insert(
        "new.jar".into(),
        File {
            bytes: b"checked".to_vec(),
            executable: false,
        },
    );
    assert!(
        clear(&spec(), false, workspace.path(), &snapshot)
            .await
            .unwrap_err()
            .to_string()
            .contains("overwrite a checked input")
    );
    assert_eq!(
        std::fs::read(workspace.path().join("new.jar")).unwrap(),
        b"checked"
    );
    snapshot.files.clear();
    clear(&spec(), false, workspace.path(), &snapshot)
        .await
        .unwrap();
    assert!(!workspace.path().join("new.jar").exists());
    clear(&spec(), true, workspace.path(), &snapshot)
        .await
        .unwrap();
    std::fs::create_dir(workspace.path().join("new.jar")).unwrap();
    assert!(
        clear(&spec(), false, workspace.path(), &snapshot)
            .await
            .is_err()
    );
    assert!(read(&workspace.path().join("new.jar")).await.is_err());
    assert!(
        read(&workspace.path().join("missing.jar"))
            .await
            .unwrap_err()
            .to_string()
            .contains("input is missing")
    );
    let oversized = workspace.path().join("large.jar");
    std::fs::File::create(&oversized)
        .unwrap()
        .set_len(compatibility::MAX_ARCHIVE_BYTES as u64 + 1)
        .unwrap();
    assert!(
        read(&oversized)
            .await
            .unwrap_err()
            .to_string()
            .contains("32 MiB")
    );
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&oversized, workspace.path().join("alias.jar")).unwrap();
        assert!(read(&workspace.path().join("alias.jar")).await.is_err());
    }
}
