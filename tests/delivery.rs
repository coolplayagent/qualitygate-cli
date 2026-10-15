mod common;
use common::*;
use qualitygate::snapshot::{self, File, Selection};
use serde_json::Value;
use std::{collections::BTreeMap, io::Read, path::Path};

fn setup() -> tempfile::TempDir {
    let root = fixture();
    std::fs::write(
        root.path().join("qualitygate.yaml"),
        "schema_version: 1\nrules: {line-ending: {}}\n",
    )
    .unwrap();
    root
}
fn contents(path: &Path) -> BTreeMap<String, File> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
    let mut files = BTreeMap::new();
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).unwrap();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        files.insert(
            entry.name().into(),
            File {
                bytes,
                executable: entry.unix_mode().unwrap() & 0o111 != 0,
            },
        );
    }
    files
}
fn export(root: &Path, destination: &Path, code: i32, args: &[&str]) -> Value {
    let mut argv = vec![
        "check",
        "--format",
        "json",
        "--export-delivery",
        destination.to_str().unwrap(),
    ];
    argv.extend_from_slice(args);
    report(&cli(root, &argv), code)
}

#[test]
fn full_gate_exports_exact_context_and_untracked_bytes_with_executable_modes() {
    let root = setup();
    let output = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("script.sh"), "#!/bin/sh\nprintf ok\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            root.path().join("script.sh"),
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();
    }
    let result = export(root.path(), output.path(), 0, &[]);
    let delivery = &result["delivery"];
    assert_eq!(delivery["snapshot"], result["report"]["snapshot"]);
    let path = Path::new(delivery["artifact"]["path"].as_str().unwrap());
    let files = contents(path);
    assert_eq!(
        snapshot::content_digest(&files),
        result["report"]["snapshot"]["content_digest"]
    );
    assert_eq!(files["hello.txt"].bytes, b"initial\n");
    assert_eq!(files["script.sh"].bytes, b"#!/bin/sh\nprintf ok\n");
    #[cfg(unix)]
    assert!(files["script.sh"].executable);
    let identity = snapshot::delivery::identify(path).unwrap();
    assert_eq!(identity.digest, delivery["artifact"]["digest"]);
    assert_eq!(identity.bytes, delivery["artifact"]["bytes"]);
    let again = export(root.path(), output.path(), 0, &[]);
    assert_eq!(again["delivery"]["artifact"], delivery["artifact"]);
}

#[test]
fn failed_incomplete_quick_and_filtered_checks_cannot_export_delivery() {
    let root = setup();
    let output = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("hello.txt"), "bad\r\n").unwrap();
    assert!(export(root.path(), output.path(), 1, &[])["delivery"].is_null());
    for args in [&["--profile", "quick"][..], &["--path", "hello.txt"][..]] {
        export(root.path(), output.path(), 2, args);
    }
    std::fs::write(root.path().join("qualitygate.yaml"), "schema_version: 1\nchecks:\n  - id: unavailable\n    kind: command\n    argv: [missing-qualitygate-delivery-producer]\n").unwrap();
    assert!(export(root.path(), output.path(), 2, &[])["delivery"].is_null());
    assert_eq!(std::fs::read_dir(output.path()).unwrap().count(), 0);
}

#[tokio::test]
async fn equivalent_repository_paths_cannot_bypass_delivery_confinement() {
    let root = setup();
    let output = tempfile::tempdir().unwrap();
    let mut captured = snapshot::capture(
        root.path(),
        &Selection::Worktree {
            base: "HEAD".into(),
        },
    )
    .await
    .unwrap();
    let nested = root.path().join("nested");
    std::fs::create_dir(&nested).unwrap();
    captured.root = nested.join("..");
    for destination in [root.path(), nested.as_path()] {
        let error = snapshot::delivery::export(&captured, destination).unwrap_err();
        assert!(error.to_string().contains("outside checked inputs"));
    }
    assert!(snapshot::delivery::export(&captured, output.path()).is_ok());
}

#[tokio::test]
async fn late_workspace_mutations_cannot_enter_the_frozen_export_and_tampering_is_rejected() {
    let root = setup();
    let output = tempfile::tempdir().unwrap();
    let captured = snapshot::capture(
        root.path(),
        &Selection::Worktree {
            base: "HEAD".into(),
        },
    )
    .await
    .unwrap();
    std::fs::write(root.path().join("hello.txt"), "unverified later mutation\n").unwrap();
    let artifact = snapshot::delivery::export(&captured, output.path()).unwrap();
    let path = Path::new(&artifact.artifact.path);
    assert_eq!(contents(path)["hello.txt"].bytes, b"initial\n");
    assert_eq!(
        snapshot::content_digest(&contents(path)),
        captured.identity.content_digest
    );
    assert!(snapshot::delivery::export(&captured, root.path()).is_err());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    #[cfg(windows)]
    {
        let mut mode = std::fs::metadata(path).unwrap().permissions();
        mode.set_readonly(false);
        std::fs::set_permissions(path, mode).unwrap();
    }
    std::fs::write(path, "substituted delivery").unwrap();
    assert_ne!(
        snapshot::delivery::identify(path).unwrap().digest,
        artifact.artifact.digest
    );
    assert!(
        snapshot::delivery::export(&captured, output.path())
            .unwrap_err()
            .to_string()
            .contains("altered")
    );
    assert!(snapshot::delivery::identify(output.path()).is_err());
    let mut inconsistent = captured.clone();
    inconsistent.files.get_mut("hello.txt").unwrap().bytes = b"unverified mutation".to_vec();
    assert!(
        snapshot::delivery::export(&inconsistent, output.path())
            .unwrap_err()
            .to_string()
            .contains("frozen snapshot identity")
    );
}
