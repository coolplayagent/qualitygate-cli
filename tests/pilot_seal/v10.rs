use super::*;

#[test]
fn cli_seals_unpriced_v10_plan_and_keeps_legacy_budget_requirement() {
    let repo = fixture();
    let root = repo.path();
    let mut candidate = v6_candidate(root);
    candidate["schema_version"] = json!(10);
    candidate["protocol"]["budget"] = Value::Null;
    candidate["protocol"]["thresholds"]
        .as_object_mut()
        .unwrap()
        .remove("cost_ratio_max");
    write(root, "candidate-v10.json", &candidate);
    let sealed = report(
        &cli(
            root,
            &[
                "pilot",
                "seal",
                "--input",
                "candidate-v10.json",
                "--format",
                "json",
            ],
        ),
        0,
    );
    assert_eq!(sealed["schema_version"], 10);
    assert!(sealed["plan_seal"]["digest"].is_string());
    assert!(sealed["protocol"]["budget"].is_null());
    assert!(
        sealed["protocol"]["thresholds"]
            .get("cost_ratio_max")
            .is_none()
    );

    candidate["schema_version"] = json!(9);
    candidate["protocol"]["thresholds"]["cost_ratio_max"] = json!(1.0);
    write(root, "unpriced-v9.json", &candidate);
    assert!(
        !cli(
            root,
            &[
                "pilot",
                "seal",
                "--input",
                "unpriced-v9.json",
                "--format",
                "json"
            ]
        )
        .status
        .success()
    );
}
