use super::super::rust_references::imports;
use std::collections::BTreeSet;

pub(super) const LEGACY_ROOTS: &[&str] = &["resource_selection", "snapshot_preflight"];

type Export = (&'static str, &'static str, &'static str);

pub(super) fn exports(root: &str) -> &'static [Export] {
    match root {
        "resource_selection" => &[
            ("exclusions", "check_planning", "snapshot_inputs"),
            (
                "resource_exclusions",
                "snapshot_scope",
                "resource_exclusions",
            ),
            (
                "selection_contracts",
                "check_planning",
                "selection_contracts",
            ),
            ("snapshot_budget", "snapshot_scope", "snapshot_budget"),
            ("trigger_selection", "check_planning", "trigger_selection"),
        ],
        "snapshot_preflight" => &[
            ("capabilities", "check_preflight", "capabilities"),
            ("doctor_output", "check_preflight", "doctor_output"),
            ("preflight_schema", "check_preflight", "preflight_schema"),
            ("readiness_check", "check_preflight", "readiness_check"),
            (
                "readiness_contracts",
                "check_preflight",
                "readiness_contracts",
            ),
            ("repository_root", "check_preflight", "repository_root"),
        ],
        "repair_feedback" => &[
            (
                "delivery_contracts",
                "delivery_export",
                "delivery_contracts",
            ),
            ("verified_delivery", "delivery_export", "verified_delivery"),
        ],
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
    let mut seen = BTreeSet::new();
    for item in items {
        let syn::Item::Use(export) = item else {
            if root == "repair_feedback" && matches!(item, syn::Item::Mod(_)) {
                continue;
            }
            return Err(format!("{root} must contain only compatibility exports"));
        };
        let mut paths = Vec::new();
        imports(&export.tree, Vec::new(), &mut paths);
        for (path, name, span) in paths {
            let matches = expected.iter().any(|(old, root, module)| {
                *old == name
                    && path
                        .iter()
                        .map(String::as_str)
                        .eq(["crate", *root, *module])
            });
            if !matches
                || !matches!(export.vis, syn::Visibility::Public(_))
                || !export.attrs.is_empty()
                || export.leading_colon.is_some()
                || !seen.insert(name.clone())
            {
                return Err(format!(
                    "line {}: unexpected compatibility export {root}::{name} from {}",
                    span.start().line,
                    path.join("::"),
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
    let (Some(source), Some(root)) = (source.first(), path.first()) else {
        return false;
    };
    let (root, module) = target(root, path.get(1).map(String::as_str));
    match (source.as_str(), root, module) {
        (
            "snapshot_scope",
            "application" | "check_inputs" | "check_planning" | "check_preflight" | "config"
            | "policy_lifecycle",
            _,
        ) => true,
        (
            "check_inputs",
            "application" | "check_preflight" | "gate_execution" | "interfaces",
            _,
        ) => true,
        ("check_inputs", "policy_lifecycle", Some(module)) => module != "active_package",
        ("policy_lifecycle", "check_inputs", _)
        | ("policy_lifecycle", "application", Some("initialization_preflight"))
        | ("repair_feedback", "delivery_export", _)
        | ("repair_feedback", "application" | "domain", Some("delivery"))
        | ("delivery_export", "repair_feedback", _)
        | ("delivery_export", "application" | "domain", Some("feedback")) => true,
        _ => false,
    }
}

#[test]
fn architecture_keeps_snapshot_scope_and_input_boundaries_acyclic() {
    for (location, source, forbidden) in [
        (
            &["snapshot_scope", "exclusions"][..],
            "use crate::resource_selection::exclusions;",
            true,
        ),
        (
            &["snapshot_scope", "exclusions"][..],
            "use crate::check_planning::snapshot_inputs;",
            true,
        ),
        (
            &["snapshot_scope", "exclusions"][..],
            "use crate::config::exclusions;",
            true,
        ),
        (
            &["snapshot_scope", "exclusions"][..],
            "use super::resource_exclusions;",
            false,
        ),
        (
            &["check_inputs"][..],
            "use crate::gate_execution::workflow;",
            true,
        ),
        (
            &["check_inputs"][..],
            "use crate::snapshot_preflight::readiness_check;",
            true,
        ),
        (
            &["check_inputs"][..],
            "use crate::policy_lifecycle::policy_loading;",
            true,
        ),
        (
            &["check_inputs"][..],
            "use crate::check_planning::check_request;",
            false,
        ),
        (
            &["policy_lifecycle", "policy_loading"][..],
            "use crate::check_inputs;",
            true,
        ),
        (
            &["policy_lifecycle", "policy_loading"][..],
            "use crate::application::initialization_preflight;",
            true,
        ),
        (
            &["policy_lifecycle", "policy_loading"][..],
            "use crate::snapshot_scope::exclusions;",
            false,
        ),
        (
            &["delivery_export", "verified_delivery"][..],
            "use crate::repair_feedback::feedback_rendering;",
            true,
        ),
        (
            &["repair_feedback", "feedback_rendering"][..],
            "use crate::repair_feedback::verified_delivery;",
            true,
        ),
    ] {
        let mut graph = super::Graph::default();
        graph.source(
            "sample.rs",
            location.iter().map(ToString::to_string).collect(),
            source,
        );
        let violations = graph.violations();
        assert_eq!(
            violations
                .iter()
                .any(|error| error.starts_with("sample.rs:1: forbidden check workflow dependency")),
            forbidden,
            "{location:?} {source}: {violations:?}",
        );
    }
}
