use super::*;

#[test]
fn rejects_unknown_fields_schemas_duplicates_and_empty_commands() {
    for yaml in [
        "schema_version: 2",
        "schema_version: 1\nrulez: {}",
        "schema_version: 1\nchecks: [{id: build}]",
        "schema_version: 1\nchecks: [{id: build, argv: [echo]}, {id: build, argv: [echo]}]",
        "schema_version: 1\nrules: {bad/id: {}}",
    ] {
        assert!(parse(yaml.as_bytes()).is_err(), "{yaml}");
    }
    assert!(parse(&vec![b' '; MAX_CONFIG_BYTES + 1]).is_err());
}

#[test]
fn validates_profiles_timeouts_thresholds_and_dependency_cycles() {
    for yaml in [
        "schema_version: 1\nrules: {format: {}}\nprofiles: {full: {include: []}}",
        "schema_version: 1\nprofiles: {typo: {include: []}}",
        "schema_version: 1\nprofiles: {quick: {include: [missing]}}",
        "schema_version: 1\nchecks: [{id: x, argv: [echo], timeout_seconds: 0}]",
        "schema_version: 1\nchecks: [{id: x, argv: [echo], depends_on: [x]}]",
        "schema_version: 1\nchecks: [{id: x, argv: [echo], depends_on: [missing]}]",
        "schema_version: 1\nchecks: [{id: x, argv: [echo], reports: [{path: report, format: lcov, minimum_coverage: 101}]}]",
        "schema_version: 1\nchecks: [{id: x, argv: [echo], reports: [{path: report, format: diagnostics, mode: new_diagnostics}]}]",
    ] {
        assert!(parse(yaml.as_bytes()).is_err(), "{yaml}");
    }
}

#[test]
fn init_detects_language_and_preserves_existing_configuration() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("Cargo.toml"), "[package]").unwrap();
    let config = init(root.path()).unwrap();
    assert_eq!(config.languages, ["rust"]);
    assert!(config.rules["line-ending"].required);
    let before = std::fs::read(root.path().join(CONFIG_FILE)).unwrap();
    init(root.path()).unwrap();
    assert_eq!(
        before,
        std::fs::read(root.path().join(CONFIG_FILE)).unwrap()
    );
    std::fs::write(root.path().join(CONFIG_FILE), "invalid").unwrap();
    assert!(init(root.path()).is_err());
    assert_eq!(
        std::fs::read_to_string(root.path().join(CONFIG_FILE)).unwrap(),
        "invalid"
    );
}

#[test]
fn valid_command_config_roundtrips_and_dependencies_need_not_be_ordered() {
    let yaml = "schema_version: 1\nchecks: [{id: test, argv: [cargo, test], depends_on: [build]}, {id: build, argv: [cargo, build]}]";
    let config = parse(yaml.as_bytes()).unwrap();
    assert_eq!(config.checks.len(), 2);
    parse(serde_norway::to_string(&config).unwrap().as_bytes()).unwrap();
}
