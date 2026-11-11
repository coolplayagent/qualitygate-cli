use super::*;

#[test]
fn early_failure_preserves_incomplete_boundary_and_quotes_literal_recovery_arguments() {
    let steps = vec![
        crate::domain::NextStep {
            action: "repair".into(),
            command: Some(vec!["qualitygate".into(), "a'b $(touch marker)".into()]),
            message: "Fix policy\u{1b}[31m".into(),
        },
        crate::domain::NextStep {
            action: "review".into(),
            command: None,
            message: "Review source".into(),
        },
    ];
    let error = "invalid policy\u{1b}[31m";
    let report: serde_json::Value = serde_json::from_str(&render_incomplete_with_steps(
        error,
        cli::Format::Json,
        &steps,
    ))
    .unwrap();
    assert_eq!(report["gate"]["complete"], false);
    assert_eq!(report["gate"]["decision"], "incomplete");
    assert_eq!(report["next_steps"][0]["command"][1], "a'b $(touch marker)");
    for format in [cli::Format::Table, cli::Format::Markdown] {
        let text = render_incomplete_with_steps(error, format, &steps);
        assert!(!text.contains('\u{1b}'));
        assert!(text.contains("complete: false"));
        assert!(text.contains("Next step: Review source"));
        assert!(text.contains("known_limits:"));
        #[cfg(not(windows))]
        assert!(text.contains("'a'\\''b $(touch marker)'"));
    }
    assert!(!render_incomplete("failed", cli::Format::Json).contains("next_steps"));
}

#[tokio::test]
async fn parser_failure_retains_machine_location_and_action_without_claiming_success() {
    let context = crate::application::policy_guidance::Context {
        root: std::path::PathBuf::from("/missing"),
        config: "qualitygate.yaml".into(),
        selection: None,
        policy_ref: false,
    };
    let error: anyhow::Error = crate::domain::runtime::ConfigurationError {
        code: "yaml.invalid".into(),
        message: "unknown field".into(),
        field: Some("rules.bad".into()),
        line: Some(7),
        column: Some(3),
    }
    .into();
    let result: serde_json::Value =
        serde_json::from_str(&render_failure(&error, cli::Format::Json, context.clone()).await)
            .unwrap();
    assert_eq!(result["diagnostics"][0]["line"], 7);
    assert_eq!(result["diagnostics"][0]["column"], 3);
    assert_eq!(result["diagnostics"][0]["field"], "rules.bad");
    assert_eq!(result["next_steps"][0]["action"], "repair_configuration");
    let text = render_failure(&error, cli::Format::Table, context.clone()).await;
    assert!(text.contains("unknown field"));
    let ordinary = anyhow::anyhow!("snapshot unavailable");
    let result: serde_json::Value =
        serde_json::from_str(&render_failure(&ordinary, cli::Format::Json, context).await).unwrap();
    assert!(result.get("diagnostics").is_none());
    assert_eq!(result["gate"]["complete"], false);
}
