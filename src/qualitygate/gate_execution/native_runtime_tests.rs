#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

use super::*;
use crate::{content_digest::sha256, snapshot::File, tool_evidence::contracts::*};
use serde_json::json;

fn spec() -> OutputSpec {
    serde_json::from_value(json!({
        "adapter":"dryer", "schema":"clone_pair", "path":"dry.edn",
        "tool_id":"dryer", "tool_version":DRYER_VERSION,
        "scope":{"project":"sample"}, "paths":["src/lib.rs"], "crate_name":"sample",
        "native_runtime":{"parser_path":"vendor/rust.so", "parser_digest":LINUX_RUST_PARSER_DIGEST, "pack_version":LANGUAGE_PACK_VERSION}
    })).unwrap()
}

fn snapshot(root: &Path) -> Snapshot {
    let mut snapshot = crate::application::test_support::snapshot(root);
    snapshot.files.insert(
        "vendor/rust.so".into(),
        File {
            bytes: b"parser".to_vec(),
            executable: false,
        },
    );
    snapshot
}

#[tokio::test]
async fn private_native_cache_pins_snapshot_bytes_and_rejects_mutation() {
    let root = tempfile::tempdir().unwrap();
    let runtime = prepare_cache(b"parser", root.path(), "dryer")
        .await
        .unwrap();
    assert_eq!(runtime.env.len(), 3);
    assert_eq!(runtime.artifacts.len(), 2);
    assert_eq!(runtime.artifacts[0].digest, sha256(b"parser"));
    assert!(runtime.env["TREE_SITTER_LANGUAGE_PACK_MANIFEST_URL"].starts_with("file://"));
    let libs = Path::new(&runtime.env["TREE_SITTER_LANGUAGE_PACK_LIBS_DIR"]);
    assert_eq!(
        tokio::fs::read(libs.join(PARSER_NAME)).await.unwrap(),
        b"parser"
    );
    assert!(!libs.parent().unwrap().join("manifest.json").exists());
    verify(Some(&runtime)).await.unwrap();
    tokio::fs::write(libs.join(PARSER_NAME), b"changed")
        .await
        .unwrap();
    assert!(verify(Some(&runtime)).await.is_err());
    tokio::fs::write(libs.join(PARSER_NAME), b"parser")
        .await
        .unwrap();
    tokio::fs::write(libs.parent().unwrap().join("manifest.json"), b"{}")
        .await
        .unwrap();
    assert!(verify(Some(&runtime)).await.is_err());
    tokio::fs::remove_file(libs.parent().unwrap().join("manifest.json"))
        .await
        .unwrap();
    tokio::fs::write(libs.parent().unwrap().join(".download.lock"), b"")
        .await
        .unwrap();
    verify(Some(&runtime)).await.unwrap();
    tokio::fs::write(libs.parent().unwrap().join(".download.lock"), b"unexpected")
        .await
        .unwrap();
    assert!(verify(Some(&runtime)).await.is_err());
    verify(None).await.unwrap();
}

#[tokio::test]
async fn native_runtime_requires_explicit_matching_snapshot_parser() {
    let root = tempfile::tempdir().unwrap();
    let mut snapshot = snapshot(root.path());
    assert!(
        prepare(&spec(), &snapshot, root.path(), "dry")
            .await
            .is_err()
    );
    let mut declaration = spec();
    declaration.native_runtime = None;
    assert!(
        prepare(&declaration, &snapshot, root.path(), "dry")
            .await
            .is_err()
    );
    let mut declaration = spec();
    declaration.native_runtime.as_mut().unwrap().parser_digest = sha256(b"wrong");
    assert!(
        prepare(&declaration, &snapshot, root.path(), "dry")
            .await
            .is_err()
    );
    snapshot
        .files
        .get_mut("vendor/rust.so")
        .unwrap()
        .bytes
        .clear();
    assert!(
        prepare(&spec(), &snapshot, root.path(), "dry")
            .await
            .is_err()
    );
    snapshot.files.clear();
    assert!(
        prepare(&spec(), &snapshot, root.path(), "dry")
            .await
            .is_err()
    );
    declaration.adapter = ToolFormat::ToolFacts;
    assert!(
        prepare(&declaration, &snapshot, root.path(), "generic")
            .await
            .unwrap()
            .is_none()
    );
}

#[cfg(unix)]
#[tokio::test]
async fn native_runtime_rejects_symlinks_and_public_cache_directories() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let root = tempfile::tempdir().unwrap();
    let runtime = prepare_cache(b"parser", root.path(), "dryer")
        .await
        .unwrap();
    let libs = Path::new(&runtime.env["TREE_SITTER_LANGUAGE_PACK_LIBS_DIR"]);
    tokio::fs::set_permissions(libs, std::fs::Permissions::from_mode(0o755))
        .await
        .unwrap();
    assert!(verify(Some(&runtime)).await.is_err());
    tokio::fs::set_permissions(libs, std::fs::Permissions::from_mode(0o700))
        .await
        .unwrap();
    tokio::fs::remove_file(libs.join(PARSER_NAME))
        .await
        .unwrap();
    symlink(&runtime.artifacts[0].path, libs.join(PARSER_NAME)).unwrap();
    assert!(verify(Some(&runtime)).await.is_err());
}
