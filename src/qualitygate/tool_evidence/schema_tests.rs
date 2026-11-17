use super::*;

#[test]
fn exported_schema_is_valid_closed_and_versioned() {
    let document = document();
    let validator = jsonschema::validator_for(&document).unwrap();
    let value = json!({"protocol_version":1,"schema":"function_metric","schema_version":1,"producer":"scan","snapshot_digest":crate::content_digest::sha256(b"snapshot"),"tool_id":"tool","tool_version":"1","artifact_digest":"unbound","scope":{"project":"demo"},"inventory":{"files":[],"entities":[]},"complete":false,"records":[]});
    assert!(validator.is_valid(&value));
    let mut invalid = value.clone();
    invalid["extra"] = json!(1);
    assert!(!validator.is_valid(&invalid));
    let mut invalid = value;
    invalid["protocol_version"] = json!(2);
    assert!(!validator.is_valid(&invalid));
    assert_eq!(
        document["$defs"]["metric"]["additionalProperties"],
        json!(false)
    );
}
