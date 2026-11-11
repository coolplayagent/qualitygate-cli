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
    ("adapters", SOURCE_ROOT_BUILD),
    ("application", SOURCE_ROOT_BUILD),
    ("config", SOURCE_ROOT_BUILD),
    ("domain", SOURCE_ROOT_BUILD),
    ("env", SOURCE_ROOT_BUILD),
    ("interfaces", SOURCE_ROOT_BUILD),
    ("net", include_str!("../src/qualitygate/net/BUILD.bazel")),
    ("paths", SOURCE_ROOT_BUILD),
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
        "report_evidence/tool_reports",
        include_str!("../src/qualitygate/report_evidence/tool_reports/BUILD.bazel"),
    ),
    (
        "source_analysis/syntax",
        include_str!("../src/qualitygate/source_analysis/syntax/BUILD.bazel"),
    ),
    (
        "check_planning/discovery",
        include_str!("../src/qualitygate/check_planning/discovery/BUILD.bazel"),
    ),
];

const VERSIONED_OWNER_BUILDS: &[&str] = &["application", "config", "interfaces", "net"];

const OWNER_DEPENDENCIES: &[(&str, &[&str])] = &[
    ("domain", &[]),
    ("env", &[]),
    ("paths", &["env"]),
    ("net", &["domain", "env"]),
    ("runner", &["domain", "env"]),
    ("snapshot", &["domain", "net", "paths", "runner"]),
    ("config", &["domain", "env", "paths"]),
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

const FLAT_OWNERS: &[&str] = &[
    "adapters",
    "application",
    "config",
    "domain",
    "env",
    "interfaces",
    "paths",
];

fn owner_label(owner: &str) -> String {
    if FLAT_OWNERS.contains(&owner) {
        format!("//src/qualitygate:{owner}")
    } else {
        format!("//src/qualitygate/{owner}")
    }
}

fn owner_build(owner: &str) -> &'static str {
    OWNER_BUILDS
        .iter()
        .find_map(|(name, source)| (*name == owner).then_some(*source))
        .unwrap_or_else(|| panic!("missing Bazel package for {owner}"))
}

fn target<'a>(source: &'a str, kind: &str, name: &str) -> &'a str {
    source
        .split(&format!("{kind}("))
        .skip(1)
        .map(|block| block.split("\n)").next().unwrap())
        .find(|block| quoted_value(block, "name") == name)
        .unwrap_or_else(|| panic!("missing {kind} target {name}"))
}

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
    assert!(MODULE.contains("isolated = False"));
    assert!(MODULE.contains("manifests = [\"//:Cargo.toml\"]"));
    assert!(CARGO_LOCK.contains("name = \"qualitygate-cli\""));
    assert!(MODULE_LOCK.contains("\"lockFileVersion\""));
    assert!(MODULE_LOCK.contains("rules_rust"));
    assert!(CARGO_ROOT_DEPS.contains("_CARGO_ROOT_PACKAGE = \"\""));
    assert!(CARGO_ROOT_DEPS.contains("package_name = _CARGO_ROOT_PACKAGE"));
    assert!(
        BUILD
            .contains("knowledge/best-practices/engineering-standards/lifecycle-rule-matrix*.yaml")
    );
    assert!(
        target(owner_build("config"), "rust_library", "config")
            .contains("//:engineering_standard_assets")
    );
    assert!(CARGO_ROOT_DEPS.contains(&format!("_CARGO_PACKAGE_VERSION = \"{package_version}\"")));
    for owner in VERSIONED_OWNER_BUILDS {
        let source = target(owner_build(owner), "rust_library", owner);
        assert!(
            source.contains("rustc_env = package_rustc_env()"),
            "Bazel {owner} must receive the Cargo package version"
        );
    }
}

#[test]
fn bazel_domain_dependencies_match_the_owner_contract() {
    for (owner, expected) in OWNER_DEPENDENCIES {
        let source = target(owner_build(owner), "rust_library", owner);
        let actual: Vec<_> = source
            .split_once("\n    deps = [")
            .map(|(_, dependencies)| dependencies.split("\n    ]").next().unwrap())
            .unwrap_or("")
            .lines()
            .map(str::trim)
            .filter_map(|line| line.strip_prefix('"'))
            .map(|label| label.trim_end_matches(',').trim_end_matches('"').to_owned())
            .collect();
        let expected: Vec<_> = expected
            .iter()
            .map(|dependency| owner_label(dependency))
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
        let flattened = FLAT_OWNERS.contains(owner);
        let source_group = if flattened {
            format!("{owner}_sources")
        } else {
            "sources".to_owned()
        };
        let sources = target(source, "filegroup", &source_group);
        let library = target(source, "rust_library", owner);
        assert_eq!(
            quoted_value(library, "crate_name"),
            format!("qualitygate_{owner}")
        );
        let entry = if flattened {
            format!("//bazel:owners/{owner}.rs")
        } else {
            "bazel_lib.rs".to_owned()
        };
        assert_eq!(quoted_value(library, "crate_root"), entry);
        assert!(library.contains(&format!("srcs = [\":{source_group}\"]")));
        if flattened {
            assert!(sources.contains(&format!("\"{owner}.rs\"")));
            assert!(sources.contains(&format!("\"{entry}\"")));
        }
        let test_name = if flattened {
            format!("{owner}_unit_tests")
        } else {
            "unit_tests".to_owned()
        };
        let unit_tests = target(source, "rust_test", &test_name);
        assert_eq!(quoted_value(unit_tests, "crate"), format!(":{owner}"));
        let test_label = if flattened {
            format!("//src/qualitygate:{test_name}")
        } else {
            format!("//src/qualitygate/{owner}:{test_name}")
        };
        assert!(BUILD.contains(&format!("\"{test_label}\"")));
    }
    for (directory, source) in NESTED_SOURCE_BUILDS {
        target(source, "filegroup", "sources");
        assert!(SOURCE_ROOT_BUILD.contains(&format!("//src/qualitygate/{directory}:sources")));
    }
    assert!(QUALITYGATE_POLICY.contains("id: bazel-module-lock"));
    assert!(QUALITYGATE_POLICY.contains("id: bazel-build"));
    assert!(QUALITYGATE_POLICY.contains("id: bazel-test"));
    // Cargo's complete integration selector includes this contract target.
    assert!(QUALITYGATE_POLICY.contains("--test, \"*\""));
}

#[test]
fn business_adapter_paths_preserve_the_public_report_contract() {
    use qualitygate::config::ReportFormat;
    use qualitygate::report_evidence::tool_reports;

    // Assignment across the two paths proves this is the same public type,
    // not a copied contract that can drift between Cargo and Bazel builds.
    let report: qualitygate::adapters::reports::Data = tool_reports::parse(
        ReportFormat::Junit,
        br#"<testsuite tests="1"><testcase name="kept"/></testsuite>"#,
    )
    .unwrap();
    assert_eq!(report.tests.unwrap().executed, 1);
    assert!(tool_reports::parse(ReportFormat::Junit, b"<testsuite").is_err());
}
