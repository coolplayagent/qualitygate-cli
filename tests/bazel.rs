//! Bazel/Cargo integration contracts that do not require a networked Bazel run.

const BAZEL_RC: &str = include_str!("../.bazelrc");
const BAZEL_IGNORE: &str = include_str!("../.bazelignore");
const BAZEL_VERSION: &str = include_str!("../.bazelversion");
const BUILD: &str = include_str!("../BUILD.bazel");
const CARGO_ROOT_DEPS: &str = include_str!("../bazel/rust_deps.bzl");
const CARGO_LOCK: &str = include_str!("../Cargo.lock");
const CARGO_MANIFEST: &str = include_str!("../Cargo.toml");
const MODULE: &str = include_str!("../MODULE.bazel");
const MODULE_LOCK: &str = include_str!("../MODULE.bazel.lock");
const QUALITYGATE_POLICY: &str = include_str!("../qualitygate.yaml");
const RUST_TOOLCHAIN: &str = include_str!("../rust-toolchain.toml");
const SOURCE_ROOT_BUILD: &str = include_str!("../src/qualitygate/BUILD.bazel");

const OWNER_BUILDS: &[(&str, &str)] = &[
    (
        "adapters",
        include_str!("../src/qualitygate/adapters/BUILD.bazel"),
    ),
    (
        "application",
        include_str!("../src/qualitygate/application/BUILD.bazel"),
    ),
    (
        "config",
        include_str!("../src/qualitygate/config/BUILD.bazel"),
    ),
    (
        "domain",
        include_str!("../src/qualitygate/domain/BUILD.bazel"),
    ),
    ("env", include_str!("../src/qualitygate/env/BUILD.bazel")),
    (
        "interfaces",
        include_str!("../src/qualitygate/interfaces/BUILD.bazel"),
    ),
    ("net", include_str!("../src/qualitygate/net/BUILD.bazel")),
    (
        "paths",
        include_str!("../src/qualitygate/paths/BUILD.bazel"),
    ),
    (
        "runner",
        include_str!("../src/qualitygate/runner/BUILD.bazel"),
    ),
    (
        "snapshot",
        include_str!("../src/qualitygate/snapshot/BUILD.bazel"),
    ),
];

const NESTED_SOURCE_BUILDS: &[(&str, &str)] = &[
    (
        "adapters/reports",
        include_str!("../src/qualitygate/adapters/reports/BUILD.bazel"),
    ),
    (
        "adapters/syntax",
        include_str!("../src/qualitygate/adapters/syntax/BUILD.bazel"),
    ),
    (
        "config/discovery",
        include_str!("../src/qualitygate/config/discovery/BUILD.bazel"),
    ),
];

const OWNER_DEPENDENCIES: &[(&str, &[&str])] = &[
    ("domain", &[]),
    ("env", &[]),
    ("paths", &["env"]),
    ("net", &["domain", "env"]),
    ("runner", &["domain", "env"]),
    ("snapshot", &["domain", "net", "paths", "runner"]),
    ("config", &["domain", "paths"]),
    ("adapters", &["config", "domain", "paths", "snapshot"]),
    (
        "application",
        &[
            "adapters", "config", "domain", "env", "net", "paths", "runner", "snapshot",
        ],
    ),
    (
        "interfaces",
        &["application", "config", "domain", "snapshot"],
    ),
];

fn quoted_value(source: &str, key: &str) -> String {
    let prefix = format!("{key} = \"");
    source
        .lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix(&prefix))
        .and_then(|line| line.split_once('"').map(|(value, _)| value.to_owned()))
        .unwrap_or_else(|| panic!("missing {key} in configuration"))
}

#[test]
fn bazel_uses_the_pinned_rust_toolchain_and_cargo_dependency_graph() {
    let package_version = quoted_value(CARGO_MANIFEST, "version");
    let rust_version = quoted_value(RUST_TOOLCHAIN, "channel");

    assert!(MODULE.contains("name = \"qualitygate_cli\""));
    assert!(MODULE.contains(&format!("version = \"{package_version}\"")));
    assert!(MODULE.contains("bazel_dep(name = \"rules_rust\""));
    assert!(MODULE.contains(&format!("versions = [\"{rust_version}\"]")));
    assert!(MODULE.contains("cargo_lockfile = \"//:Cargo.lock\""));
    assert!(MODULE.contains("manifests = [\"//:Cargo.toml\"]"));
    assert!(CARGO_LOCK.contains("name = \"qualitygate-cli\""));
    assert!(MODULE_LOCK.contains("\"lockFileVersion\""));
    assert!(MODULE_LOCK.contains("rules_rust"));
    assert!(CARGO_ROOT_DEPS.contains("_CARGO_ROOT_PACKAGE = \"\""));
    assert!(CARGO_ROOT_DEPS.contains("package_name = _CARGO_ROOT_PACKAGE"));
}

#[test]
fn bazel_domain_dependencies_match_the_owner_contract() {
    for (owner, expected) in OWNER_DEPENDENCIES {
        let source = OWNER_BUILDS
            .iter()
            .find_map(|(name, source)| (*name == *owner).then_some(*source))
            .unwrap_or_else(|| panic!("missing Bazel package for {owner}"));
        let actual: Vec<_> = source
            .lines()
            .map(str::trim)
            .map(|line| line.trim_end_matches(',').trim_matches('"'))
            .filter(|label| label.starts_with("//src/qualitygate/") && !label.contains(':'))
            .map(str::to_owned)
            .collect();
        let expected: Vec<_> = expected
            .iter()
            .map(|dependency| format!("//src/qualitygate/{dependency}"))
            .collect();
        assert_eq!(
            actual, expected,
            "Bazel dependencies for {owner} diverge from the ownership contract"
        );
    }
}

#[test]
fn bazel_exposes_cached_domain_libraries_and_dependency_contract_test() {
    assert_eq!(BAZEL_VERSION.trim(), "9.2.0");
    assert!(BAZEL_IGNORE.lines().any(|line| line == "target"));
    assert!(BAZEL_RC.contains("--repository_cache=~/.cache/qualitygate-cli/bazel/repositories"));
    assert!(BAZEL_RC.contains("--disk_cache=~/.cache/qualitygate-cli/bazel/actions"));
    assert!(BUILD.contains("name = \"qualitygate\""));
    assert!(BUILD.contains("name = \"qualitygate_lib\""));
    assert!(BUILD.contains("name = \"bazel_contract_test\""));
    assert!(BUILD.contains("test_suite("));
    assert!(!BUILD.contains("glob([\"src/qualitygate/**/*.rs\"])"));
    assert!(SOURCE_ROOT_BUILD.contains("name = \"qualitygate_lib\""));
    assert!(SOURCE_ROOT_BUILD.contains("crate_root = \"bazel_lib.rs\""));
    for (owner, source) in OWNER_BUILDS {
        assert!(
            source.contains("name = \"sources\""),
            "missing source group for {owner}"
        );
        assert!(
            source.contains("rust_library("),
            "missing library target for {owner}"
        );
        assert!(
            source.contains("crate_root = \"bazel_lib.rs\""),
            "missing domain entry point for {owner}"
        );
    }
    for (directory, source) in NESTED_SOURCE_BUILDS {
        assert!(
            source.contains("name = \"sources\""),
            "missing nested source group for {directory}"
        );
    }
    assert!(QUALITYGATE_POLICY.contains("id: bazel-module-lock"));
    assert!(QUALITYGATE_POLICY.contains("id: bazel-build"));
    assert!(QUALITYGATE_POLICY.contains("id: bazel-test"));
    assert!(QUALITYGATE_POLICY.contains("--test, bazel"));
}
