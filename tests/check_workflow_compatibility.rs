//! Check workflow paths shared by the Cargo library and the Bazel aggregate.

use qualitygate::{
    application, check_planning, check_preflight, config, delivery_export, domain, repair_feedback,
    resource_selection, snapshot_preflight, snapshot_scope,
};

#[test]
fn check_workflow_paths_preserve_public_module_identity() {
    let report: check_preflight::readiness_contracts::Report =
        snapshot_preflight::readiness_contracts::Report::new("full".into(), false);
    let report: domain::doctor::Report = report;
    assert_eq!(report.scope, "preflight");
    let _ = [
        check_preflight::readiness_check::run,
        snapshot_preflight::readiness_check::run,
        application::doctor::run,
    ];

    let triggers: check_planning::selection_contracts::WhenChanged =
        resource_selection::selection_contracts::WhenChanged::default();
    let _: domain::change_selection::WhenChanged = triggers;
    let patterns = vec!["cache/**".into()];
    for matcher in [
        snapshot_scope::exclusions::matcher,
        resource_selection::exclusions::matcher,
        config::exclusions::matcher,
    ] {
        assert!(matcher(&patterns).unwrap().is_match("cache/index"));
    }

    let delivery: Option<delivery_export::delivery_contracts::Delivery> =
        None::<repair_feedback::delivery_contracts::Delivery>;
    let _: Option<domain::delivery::Delivery> = delivery;
    let _ = [
        delivery_export::verified_delivery::check,
        repair_feedback::verified_delivery::check,
        application::delivery::check,
    ];
}
