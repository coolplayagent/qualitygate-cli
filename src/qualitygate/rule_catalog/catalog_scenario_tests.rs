use super::*;
use serde_json::json;

#[test]
fn project_defaults_and_lifecycle_are_resolved_without_erasing_explicit_overrides() {
    let source = json!({"document":"AGENTS.md","section":"Rules","content_hash":crate::config::policy_store::digest(b"rule")});
    let rule:CustomRule=serde_json::from_value(json!({"id":"scenario","version":2,"source":source,"language":["rust"],"requires_capabilities":["test_methods"],"when":{"entity":"test_method"},"then":{"min_count":1},"fix":"Add behavior tests","required":false,"severity":"warning"})).unwrap();
    let catalog = Catalog {
        entries: [(
            "scenario".into(),
            Entry {
                origin: "project".into(),
                package: "qualitygate/rules/scenario.yaml".into(),
                builtin: None,
                custom: Some(rule),
            },
        )]
        .into(),
    };
    assert_eq!(catalog.entries["scenario"].version(), 2);
    let mut config: Config =
        serde_norway::from_slice(b"schema_version: 1\nrules:\n  scenario: {}\n").unwrap();
    let resolved = catalog.resolve(&config).unwrap();
    assert!(!resolved.rules["scenario"].required);
    assert_eq!(
        resolved.rules["scenario"].severity,
        crate::domain::Severity::Warning
    );
    config
        .rules
        .get_mut("scenario")
        .unwrap()
        .parameters
        .insert("unsupported".into(), json!(true));
    assert!(
        catalog
            .resolve(&config)
            .unwrap_err()
            .to_string()
            .contains("parameters are unsupported")
    );
    config.rules.get_mut("scenario").unwrap().parameters.clear();
    for state in ["revalidate", "demoted", "deprecated", "retired", "revoked"] {
        config.rule_lifecycle.insert("scenario".into(),serde_json::from_value(json!({"state":state,"actor":{"id":"reviewer","kind":"human"},"reason":"Observed policy drift","evidence_refs":[crate::config::policy_store::digest(b"review")],"timestamp":1})).unwrap());
        let resolved = catalog.resolve(&config).unwrap();
        let setting = &resolved.rules["scenario"];
        assert_eq!(setting.enabled, !matches!(state, "retired" | "revoked"));
        if state == "demoted" {
            assert!(!setting.required);
            assert_eq!(setting.severity, crate::domain::Severity::Warning);
        }
    }
}

#[test]
fn standards_review_dates_obey_real_calendar_and_bounded_iso_format() {
    for valid in [
        "2000-02-29",
        "2024-02-29",
        "2100-02-28",
        "2026-04-30",
        "2026-12-31",
        "9999-01-01",
    ] {
        assert!(valid_date(valid), "{valid}");
    }
    for invalid in [
        "2026",
        "2026-01",
        "2026-01-01-extra",
        "2026-1-01",
        "year-01-01",
        "1999-12-31",
        "2026-00-01",
        "2026-13-01",
        "2026-02-29",
        "2100-02-29",
        "2026-04-31",
        "2026-01-00",
    ] {
        assert!(!valid_date(invalid), "{invalid}");
    }
}

#[test]
fn bundled_standard_references_cannot_claim_unmapped_controls_or_lifecycle_inputs() {
    let standards =
        standards_from(STANDARD_REGISTRY, LIFECYCLE_MATRIX, LIFECYCLE_SUPPLEMENTS).unwrap();
    let config = crate::config::parse(b"schema_version: 1\n").unwrap();
    let catalog = Catalog::load(&config, []).unwrap();
    let builtin = catalog.entries["line-ending"].builtin.as_ref().unwrap();
    assert!(
        !validate_builtin_standards(builtin, &standards)
            .unwrap()
            .is_empty()
    );
    for scenario in ["absent", "unknown-input", "duplicate-input", "wrong-source"] {
        let mut rule = builtin.clone();
        match scenario {
            "absent" => rule.standard_refs.clear(),
            "unknown-input" => rule.lifecycle_inputs = vec!["invented-control".into()],
            "duplicate-input" => rule.lifecycle_inputs.push(rule.lifecycle_inputs[0].clone()),
            _ => {
                rule.standard_refs[0].source_id = "unarchived-standard".into();
            }
        }
        let error = validate_builtin_standards(&rule, &standards)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains(match scenario {
                "absent" => "must declare",
                "unknown-input" => "unknown lifecycle",
                "duplicate-input" => "invalid lifecycle",
                _ => "unsupported standard control",
            }),
            "{error}"
        );
    }
}

#[test]
fn builtin_presentation_is_validated_even_before_a_rule_is_selected() {
    let standards =
        standards_from(STANDARD_REGISTRY, LIFECYCLE_MATRIX, LIFECYCLE_SUPPLEMENTS).unwrap();
    let catalog = Catalog::load(&Config::default(), []).unwrap();
    let mut rule = catalog.entries["workaround-marker"]
        .builtin
        .as_ref()
        .unwrap()
        .clone();
    validate_builtin_standards(&rule, &standards).unwrap();
    rule.diagnostic.as_mut().unwrap().fix = " ".into();
    assert!(validate_builtin_standards(&rule, &standards).is_err());
    rule.diagnostic.as_mut().unwrap().fix = "Inspect the source".into();
    rule.implementation = "line-ending".into();
    assert!(validate_builtin_standards(&rule, &standards).is_err());
}
