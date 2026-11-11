use super::*;
use std::path::Path;

#[test]
fn missing_oversized_and_nonregular_configuration_remain_distinct_from_parse_errors() {
    let root = tempfile::tempdir().unwrap();
    let path = Path::new("qualitygate.yaml");
    let error = read(root.path(), path).unwrap_err();
    assert!(
        error
            .downcast_ref::<crate::domain::PolicyInputError>()
            .is_some()
    );
    std::fs::create_dir(root.path().join(path)).unwrap();
    assert!(read(root.path(), path).is_err());
    std::fs::remove_dir(root.path().join(path)).unwrap();
    std::fs::write(root.path().join(path), vec![b' '; MAX_CONFIG_BYTES + 1]).unwrap();
    assert!(read(root.path(), path).is_err());
    let denied = read_error(
        path,
        std::io::Error::from(std::io::ErrorKind::PermissionDenied),
    );
    assert!(denied.to_string().contains("Cannot read"));
    std::fs::write(root.path().join(path), "schema_version: 1\n").unwrap();
    enable_rule(root.path(), path, "line-ending").unwrap();
    assert!(read(root.path(), path).unwrap().rules["line-ending"].enabled);
}

#[test]
fn initialization_with_reviewable_checks_protects_inputs_and_never_replaces_existing_policy() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(
        root.path().join("Cargo.toml"),
        "[package]\nname = 'unit-example'\nversion = '0.1.0'\nedition = '2024'\n",
    )
    .unwrap();
    std::fs::create_dir(root.path().join("src")).unwrap();
    std::fs::write(root.path().join("src/lib.rs"), "pub fn behavior() {}\n").unwrap();
    let path = Path::new("config/qualitygate.yaml");
    let initialized = initialization::initialize_at(root.path(), path, true).unwrap();
    assert!(initialized.created);
    assert!(initialized.with_checks_applied);
    assert!(!initialized.config.checks.is_empty());
    assert!(
        initialized
            .config
            .verification_assets
            .contains(&"Cargo.toml".into())
    );
    let original = std::fs::read(root.path().join(path)).unwrap();
    let repeated = initialization::initialize_at(root.path(), path, true).unwrap();
    assert!(!repeated.created);
    assert!(!repeated.with_checks_applied);
    assert_eq!(std::fs::read(root.path().join(path)).unwrap(), original);
}
