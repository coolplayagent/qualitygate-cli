use super::*;

fn plan() -> Manifest {
    let (mut manifest, _) = super::v8::plan();
    manifest.schema_version = 10;
    manifest.protocol.budget = None;
    manifest.protocol.thresholds.cost_ratio_max = None;
    manifest
}

#[test]
fn v10_seals_without_prices_and_preserves_the_declared_trial_controls() {
    let manifest = plan();
    validate(&manifest).unwrap();
    let sealed = seal(manifest, 1).unwrap();
    assert_eq!(sealed.schema_version, 10);
    assert!(sealed.plan_seal.is_some());
    let value = serde_json::to_value(&sealed).unwrap();
    assert!(value["protocol"]["budget"].is_null());
    assert!(
        value["protocol"]["thresholds"]
            .get("cost_ratio_max")
            .is_none()
    );
    let summary = summarize(&sealed, &Reports::new(), 1).unwrap();
    assert!(summary["budget"].is_null());
    assert!(
        !summary["limitations"]
            .to_string()
            .contains("monetary budget")
    );
    assert_eq!(summary["protocol_ready"], false);
}

#[test]
fn v10_rejects_financial_controls_and_v9_still_requires_them() {
    let manifest = plan();
    let mut invalid = manifest.clone();
    invalid.protocol.thresholds.cost_ratio_max = Some(1.0);
    assert!(validate(&invalid).is_err());
    invalid = manifest.clone();
    invalid.protocol.monetary_cap = Some("USD 1".into());
    assert!(validate(&invalid).is_err());
    invalid = manifest.clone();
    invalid.schema_version = 9;
    invalid.protocol.thresholds.cost_ratio_max = Some(1.0);
    assert!(validate(&invalid).is_err());
    invalid = manifest;
    invalid.protocol.thresholds.full_p95_ratio_max = 0.0;
    assert!(validate(&invalid).is_err());
}

#[test]
fn v10_keeps_attempt_model_evidence_mandatory() {
    let (mut manifest, reports) = super::v8::plan();
    manifest.schema_version = 10;
    manifest.protocol.budget = None;
    manifest.protocol.thresholds.cost_ratio_max = None;
    let mut manifest = seal(manifest, 1).unwrap();
    super::v8::observed(&mut manifest);
    let present = summarize(&manifest, &reports, 604803).unwrap();
    assert!(
        !present["limitations"]
            .to_string()
            .contains("Observed attempts lack archived model capture evidence")
    );

    manifest.observations[0].attempts[0].model_evidence = None;
    let missing = summarize(&manifest, &reports, 604803).unwrap();
    assert_eq!(missing["model_audit"]["missing_attempts"], 1);
    assert!(
        missing["limitations"]
            .to_string()
            .contains("Observed attempts lack archived model capture evidence")
    );
    assert_eq!(missing["complete"], false);
}
