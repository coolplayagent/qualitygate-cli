//! Serializable fixture outcomes and pure golden comparisons.

use super::{Decision, VerificationBoundary};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssertionMismatch {
    pub assertion: String,
    pub expected: Value,
    pub actual: Option<Value>,
}

pub fn compare_golden(
    expected: &BTreeMap<String, Value>,
    actual: &Value,
) -> Vec<AssertionMismatch> {
    expected
        .iter()
        .filter_map(|(pointer, expected)| {
            let observed = actual.pointer(pointer);
            let matches = if let Some(fragment) = expected.get("$contains").and_then(Value::as_str)
            {
                observed
                    .and_then(Value::as_str)
                    .is_some_and(|text| text.contains(fragment))
            } else {
                observed == Some(expected)
            };
            (!matches).then(|| AssertionMismatch {
                assertion: pointer.clone(),
                expected: expected.clone(),
                actual: observed.cloned(),
            })
        })
        .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixtureResult {
    pub fixture: String,
    pub suite: String,
    pub rule: String,
    pub input: String,
    pub input_digest: String,
    pub golden_digest: String,
    pub decision: Decision,
    pub mismatches: Vec<AssertionMismatch>,
    pub error: Option<String>,
    pub observed: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelfcheckReport {
    pub schema_version: u32,
    pub tool_version: String,
    pub corpus_digest: String,
    pub rules_digest: String,
    pub fixture_filter: Option<String>,
    pub rule_filter: Option<String>,
    pub decision: Decision,
    pub complete: bool,
    pub fixtures: Vec<FixtureResult>,
    pub errors: Vec<String>,
    pub verification: VerificationBoundary,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn golden_comparison_distinguishes_missing_null_and_wrong_values() {
        let expected = BTreeMap::from([
            ("/verdict".into(), Value::Null),
            ("/diagnostics/0/file".into(), json!("a.rs")),
        ]);
        assert!(
            compare_golden(
                &expected,
                &json!({"verdict":null,"diagnostics":[{"file":"a.rs"}]})
            )
            .is_empty()
        );
        let mismatch = compare_golden(&expected, &json!({"verdict":"pass"}));
        assert_eq!(mismatch.len(), 2);
        assert_eq!(mismatch[0].actual, None);
        assert_eq!(mismatch[1].actual, Some(json!("pass")));
    }

    #[test]
    fn native_reason_assertions_require_the_expected_fragment_in_a_string() {
        let expected = BTreeMap::from([("/reason".into(), json!({"$contains":"missing-tool"}))]);
        assert!(
            compare_golden(
                &expected,
                &json!({"reason":"Cannot start /tmp/missing-tool"})
            )
            .is_empty()
        );
        for actual in [
            json!({"reason":"different error"}),
            json!({"reason":null}),
            json!({}),
        ] {
            assert_eq!(compare_golden(&expected, &actual).len(), 1);
        }
    }
}
