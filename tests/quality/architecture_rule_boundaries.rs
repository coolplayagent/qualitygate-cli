use super::super::rust_references::imports;
use std::collections::BTreeSet;

pub(super) const LEGACY_ROOT: &str = "rule_evaluation";

// Each tuple gives the old exported name and the concrete root and module.
const LEGACY_EXPORTS: &[(&str, &str, &str)] = &[
    ("builtin_rules", "builtin_rules", "dispatch"),
    ("custom_rules", "rule_engine", "custom_rules"),
    ("evaluation_facts", "rule_engine", "evaluation_facts"),
    ("operators_v2", "rule_engine", "operators_v2"),
    ("operators_v3", "rule_engine", "operators_v3"),
    ("operators_v4", "rule_engine", "operators_v4"),
    ("operators_v4_facts", "rule_engine", "operators_v4_facts"),
];
const ADAPTER_EXPORTS: &[(&str, &str, &str)] = &[
    ("rules", "builtin_rules", "dispatch"),
    ("custom_rules", "rule_engine", "custom_rules"),
    ("facts", "rule_engine", "evaluation_facts"),
    ("operators", "rule_engine", "operators_v2"),
    ("operators_v3", "rule_engine", "operators_v3"),
];

fn exports(root: &str) -> &'static [(&'static str, &'static str, &'static str)] {
    match root {
        LEGACY_ROOT => LEGACY_EXPORTS,
        "adapters" => ADAPTER_EXPORTS,
        _ => &[],
    }
}

pub(super) fn target<'a>(root: &'a str, module: Option<&'a str>) -> (&'a str, Option<&'a str>) {
    exports(root)
        .iter()
        .find(|(old, _, _)| Some(*old) == module)
        .map_or((root, module), |(_, root, module)| (root, Some(module)))
}

pub(super) fn validate_exports(root: &str, items: &[syn::Item]) -> Result<(), String> {
    let expected = exports(root);
    let strict = root == LEGACY_ROOT;
    let mut seen = BTreeSet::new();
    for item in items {
        let syn::Item::Use(export) = item else {
            if strict {
                return Err("rule_evaluation must contain only compatibility exports".into());
            }
            continue;
        };
        let mut paths = Vec::new();
        imports(&export.tree, Vec::new(), &mut paths);
        for (path, old, span) in paths {
            let specified = expected.iter().find(|(name, _, _)| *name == old);
            let rule_path = path.first().is_some_and(|part| part == "crate")
                && path.get(1).is_some_and(|part| {
                    matches!(part.as_str(), LEGACY_ROOT | "rule_engine" | "builtin_rules")
                });
            if !strict && !rule_path && specified.is_none() {
                continue;
            }
            let matches = specified.is_some_and(|(_, root, module)| {
                path.iter()
                    .map(String::as_str)
                    .eq(["crate", *root, *module])
            });
            if !matches
                || !matches!(export.vis, syn::Visibility::Public(_))
                || !export.attrs.is_empty()
                || export.leading_colon.is_some()
                || !seen.insert(old.clone())
            {
                return Err(format!(
                    "line {}: unexpected compatibility export {old:?} from {}",
                    span.start().line,
                    path.join("::")
                ));
            }
        }
    }
    for (name, _, _) in expected {
        if !seen.contains(*name) {
            return Err(format!("missing compatibility export {root}::{name}"));
        }
    }
    Ok(())
}

pub(super) fn forbidden_dependency(source: &[String], path: &[String]) -> bool {
    let (Some(source_root), Some(root)) = (source.first(), path.first()) else {
        return false;
    };
    let module = path.get(1).map(String::as_str);
    let (target_root, target_module) = target(root, module);
    let compatibility_glob = matches!(
        (target_root, target_module),
        ("adapters" | LEGACY_ROOT, Some("*"))
    );
    if source_root == "external_acceptance"
        && source
            .get(1)
            .is_some_and(|module| module == "trailer_analysis")
    {
        return target_root == "rule_engine" || compatibility_glob;
    }
    match (source_root.as_str(), target_root, target_module) {
        ("rule_engine", "builtin_rules", _) => true,
        ("builtin_rules", "rule_engine", Some("evaluation_facts" | "markers")) => false,
        ("builtin_rules", "rule_engine", _) => true,
        ("rule_engine" | "builtin_rules", _, _) => compatibility_glob,
        _ => false,
    }
}

#[test]
fn architecture_checks_exact_public_rule_compatibility_exports() {
    let valid = r#"
        pub use crate::builtin_rules::dispatch as builtin_rules;
        pub use crate::rule_engine::{
            custom_rules, evaluation_facts, operators_v2, operators_v3, operators_v4,
            operators_v4_facts,
        };
    "#;
    for (change, expected) in [
        ("", None),
        (
            "fn private_implementation() {}",
            Some("only compatibility exports"),
        ),
        (
            "pub use crate::rule_engine::*;",
            Some("unexpected compatibility export"),
        ),
        (
            "pub use crate::builtin_rules::dispatch as another;",
            Some("unexpected compatibility export"),
        ),
    ] {
        let parsed = syn::parse_file(&format!("{valid}{change}")).unwrap();
        let result = validate_exports(LEGACY_ROOT, &parsed.items);
        if let Some(expected) = expected {
            assert!(result.unwrap_err().contains(expected));
        } else {
            result.unwrap();
        }
    }
    let altered = valid.replace(
        "dispatch as builtin_rules",
        "project_checks as builtin_rules",
    );
    let parsed = syn::parse_file(&altered).unwrap();
    assert!(
        validate_exports(LEGACY_ROOT, &parsed.items)
            .unwrap_err()
            .contains("unexpected compatibility export")
    );

    let adapter = r#"
        pub use crate::builtin_rules::dispatch as rules;
        pub use crate::rule_engine::{
            custom_rules, evaluation_facts as facts, operators_v2 as operators, operators_v3,
        };
        pub use crate::report_evidence::diagnostic_budget;
    "#;
    let parsed = syn::parse_file(adapter).unwrap();
    validate_exports("adapters", &parsed.items).unwrap();
    let altered = adapter.replace("dispatch as rules", "dispatch as hidden");
    let parsed = syn::parse_file(&altered).unwrap();
    assert!(
        validate_exports("adapters", &parsed.items)
            .unwrap_err()
            .contains("unexpected compatibility export")
    );
}
