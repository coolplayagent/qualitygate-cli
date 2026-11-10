use super::*;
use serde_json::json;

#[test]
fn report_contracts_reject_ambiguous_producers_missing_denominators_and_test_ratchets() {
    let check =
        json!({"id":"verify","argv":["tool"],"tools":[{"id":"tool","argv":["tool","--version"]}]});
    for report in [
        json!({"path":"clippy.json","format":"cargo_clippy"}),
        json!({"path":"tests.xml","format":"junit","minimum_tests":0}),
        json!({"path":"tests.xml","format":"junit","ratchet":true}),
        json!({"path":"coverage.json","format":"coverage","coverage_paths":[]}),
        json!({"path":"coverage.json","format":"coverage","coverage_paths":["src/**"],"coverage_mode":"invalid"}),
        json!({"path":"../report.json","format":"diagnostics"}),
    ] {
        let mut selected = check.clone();
        selected["reports"] = json!([report]);
        let config = serde_json::from_value(json!({"schema_version":1,"checks":[selected]}));
        assert!(config.is_err() || validate(&config.unwrap()).is_err());
    }
    let mut selected = check;
    selected["reports"] = json!([{"path":"one.json","format":"diagnostics","from_stdout":true},{"path":"two.json","format":"diagnostics","from_stdout":true}]);
    let config = serde_json::from_value(json!({"schema_version":1,"checks":[selected]})).unwrap();
    assert!(
        validate(&config)
            .unwrap_err()
            .to_string()
            .contains("one stdout")
    );
}

#[test]
fn manual_and_provenance_evidence_must_be_confined_and_cannot_hide_executable_work() {
    for check in [
        json!({"id":"review","kind":"manual","argv":["execute"]}),
        json!({"id":"review","kind":"manual","cwd":"subdir"}),
        json!({"id":"review","kind":"manual","evidence_file":"../outside.json"}),
        json!({"id":"review","argv":["tool"],"evidence_file":"review.json"}),
    ] {
        let config = serde_json::from_value(json!({"schema_version":1,"checks":[check]})).unwrap();
        assert!(validate(&config).is_err());
    }
    for path in ["", "../outside.json", "/external.json"] {
        let config=serde_json::from_value(json!({"schema_version":1,"rules":{"line-ending":{"provenance":{"evidence_file":path}}}})).unwrap();
        assert!(validate(&config).is_err());
    }
    let config=serde_json::from_value(json!({"schema_version":1,"rules":{"line-ending":{"provenance":{"evidence_file":"provenance.json"}}}})).unwrap();
    validate(&config).unwrap();
}
