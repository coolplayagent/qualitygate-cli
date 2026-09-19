//! Versioned, compiled fixture inputs; goldens are authored independently.

use super::{CustomRule, ReportFormat};
use crate::domain::{CheckResult, ProjectFacts};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub const SUITES: &[(&str, &str, &str, &str, &str)] = &[
    (
        "minimal",
        "fixtures/minimal/cases.json",
        include_str!("../../../fixtures/minimal/cases.json"),
        "fixtures/golden/minimal.json",
        include_str!("../../../fixtures/golden/minimal.json"),
    ),
    (
        "minimal",
        "fixtures/minimal/issue9-cases.json",
        include_str!("../../../fixtures/minimal/issue9-cases.json"),
        "fixtures/golden/issue9-minimal.json",
        include_str!("../../../fixtures/golden/issue9-minimal.json"),
    ),
    (
        "minimal",
        "fixtures/minimal/issue10-cases.json",
        include_str!("../../../fixtures/minimal/issue10-cases.json"),
        "fixtures/golden/issue10-minimal.json",
        include_str!("../../../fixtures/golden/issue10-minimal.json"),
    ),
    (
        "minimal",
        "fixtures/minimal/issue11-cases.json",
        include_str!("../../../fixtures/minimal/issue11-cases.json"),
        "fixtures/golden/issue11-minimal.json",
        include_str!("../../../fixtures/golden/issue11-minimal.json"),
    ),
    (
        "minimal",
        "fixtures/minimal/issue12-cases.json",
        include_str!("../../../fixtures/minimal/issue12-cases.json"),
        "fixtures/golden/issue12-minimal.json",
        include_str!("../../../fixtures/golden/issue12-minimal.json"),
    ),
    (
        "minimal",
        "fixtures/minimal/issue13-cases.json",
        include_str!("../../../fixtures/minimal/issue13-cases.json"),
        "fixtures/golden/issue13-minimal.json",
        include_str!("../../../fixtures/golden/issue13-minimal.json"),
    ),
    (
        "minimal",
        "fixtures/minimal/issue14-cases.json",
        include_str!("../../../fixtures/minimal/issue14-cases.json"),
        "fixtures/golden/issue14-minimal.json",
        include_str!("../../../fixtures/golden/issue14-minimal.json"),
    ),
    (
        "minimal",
        "fixtures/minimal/issue15-cases.json",
        include_str!("../../../fixtures/minimal/issue15-cases.json"),
        "fixtures/golden/issue15-minimal.json",
        include_str!("../../../fixtures/golden/issue15-minimal.json"),
    ),
    (
        "minimal",
        "fixtures/minimal/issue16-cases.json",
        include_str!("../../../fixtures/minimal/issue16-cases.json"),
        "fixtures/golden/issue16-minimal.json",
        include_str!("../../../fixtures/golden/issue16-minimal.json"),
    ),
    (
        "minimal",
        "fixtures/minimal/issue18-cases.json",
        include_str!("../../../fixtures/minimal/issue18-cases.json"),
        "fixtures/golden/issue18-minimal.json",
        include_str!("../../../fixtures/golden/issue18-minimal.json"),
    ),
    (
        "minimal",
        "fixtures/minimal/issue19-cases.json",
        include_str!("../../../fixtures/minimal/issue19-cases.json"),
        "fixtures/golden/issue19-minimal.json",
        include_str!("../../../fixtures/golden/issue19-minimal.json"),
    ),
    (
        "minimal",
        "fixtures/minimal/issue20-cases.json",
        include_str!("../../../fixtures/minimal/issue20-cases.json"),
        "fixtures/golden/issue20-minimal.json",
        include_str!("../../../fixtures/golden/issue20-minimal.json"),
    ),
    (
        "typical",
        "fixtures/typical/cases.json",
        include_str!("../../../fixtures/typical/cases.json"),
        "fixtures/golden/typical.json",
        include_str!("../../../fixtures/golden/typical.json"),
    ),
    (
        "typical",
        "fixtures/typical/issue9-cases.json",
        include_str!("../../../fixtures/typical/issue9-cases.json"),
        "fixtures/golden/issue9-typical.json",
        include_str!("../../../fixtures/golden/issue9-typical.json"),
    ),
    (
        "typical",
        "fixtures/typical/issue10-cases.json",
        include_str!("../../../fixtures/typical/issue10-cases.json"),
        "fixtures/golden/issue10-typical.json",
        include_str!("../../../fixtures/golden/issue10-typical.json"),
    ),
    (
        "typical",
        "fixtures/typical/issue11-cases.json",
        include_str!("../../../fixtures/typical/issue11-cases.json"),
        "fixtures/golden/issue11-typical.json",
        include_str!("../../../fixtures/golden/issue11-typical.json"),
    ),
    (
        "typical",
        "fixtures/typical/issue12-cases.json",
        include_str!("../../../fixtures/typical/issue12-cases.json"),
        "fixtures/golden/issue12-typical.json",
        include_str!("../../../fixtures/golden/issue12-typical.json"),
    ),
    (
        "typical",
        "fixtures/typical/issue13-cases.json",
        include_str!("../../../fixtures/typical/issue13-cases.json"),
        "fixtures/golden/issue13-typical.json",
        include_str!("../../../fixtures/golden/issue13-typical.json"),
    ),
    (
        "typical",
        "fixtures/typical/issue14-cases.json",
        include_str!("../../../fixtures/typical/issue14-cases.json"),
        "fixtures/golden/issue14-typical.json",
        include_str!("../../../fixtures/golden/issue14-typical.json"),
    ),
    (
        "typical",
        "fixtures/typical/issue15-cases.json",
        include_str!("../../../fixtures/typical/issue15-cases.json"),
        "fixtures/golden/issue15-typical.json",
        include_str!("../../../fixtures/golden/issue15-typical.json"),
    ),
    (
        "typical",
        "fixtures/typical/issue16-cases.json",
        include_str!("../../../fixtures/typical/issue16-cases.json"),
        "fixtures/golden/issue16-typical.json",
        include_str!("../../../fixtures/golden/issue16-typical.json"),
    ),
    (
        "typical",
        "fixtures/typical/issue18-cases.json",
        include_str!("../../../fixtures/typical/issue18-cases.json"),
        "fixtures/golden/issue18-typical.json",
        include_str!("../../../fixtures/golden/issue18-typical.json"),
    ),
    (
        "typical",
        "fixtures/typical/issue19-cases.json",
        include_str!("../../../fixtures/typical/issue19-cases.json"),
        "fixtures/golden/issue19-typical.json",
        include_str!("../../../fixtures/golden/issue19-typical.json"),
    ),
    (
        "typical",
        "fixtures/typical/issue20-cases.json",
        include_str!("../../../fixtures/typical/issue20-cases.json"),
        "fixtures/golden/issue20-typical.json",
        include_str!("../../../fixtures/golden/issue20-typical.json"),
    ),
    (
        "stress",
        "fixtures/stress/cases.json",
        include_str!("../../../fixtures/stress/cases.json"),
        "fixtures/golden/stress.json",
        include_str!("../../../fixtures/golden/stress.json"),
    ),
    (
        "stress",
        "fixtures/stress/issue9-cases.json",
        include_str!("../../../fixtures/stress/issue9-cases.json"),
        "fixtures/golden/issue9-stress.json",
        include_str!("../../../fixtures/golden/issue9-stress.json"),
    ),
    (
        "stress",
        "fixtures/stress/issue10-cases.json",
        include_str!("../../../fixtures/stress/issue10-cases.json"),
        "fixtures/golden/issue10-stress.json",
        include_str!("../../../fixtures/golden/issue10-stress.json"),
    ),
    (
        "stress",
        "fixtures/stress/issue11-cases.json",
        include_str!("../../../fixtures/stress/issue11-cases.json"),
        "fixtures/golden/issue11-stress.json",
        include_str!("../../../fixtures/golden/issue11-stress.json"),
    ),
    (
        "stress",
        "fixtures/stress/issue12-cases.json",
        include_str!("../../../fixtures/stress/issue12-cases.json"),
        "fixtures/golden/issue12-stress.json",
        include_str!("../../../fixtures/golden/issue12-stress.json"),
    ),
    (
        "stress",
        "fixtures/stress/issue13-cases.json",
        include_str!("../../../fixtures/stress/issue13-cases.json"),
        "fixtures/golden/issue13-stress.json",
        include_str!("../../../fixtures/golden/issue13-stress.json"),
    ),
    (
        "stress",
        "fixtures/stress/issue14-cases.json",
        include_str!("../../../fixtures/stress/issue14-cases.json"),
        "fixtures/golden/issue14-stress.json",
        include_str!("../../../fixtures/golden/issue14-stress.json"),
    ),
    (
        "stress",
        "fixtures/stress/issue15-cases.json",
        include_str!("../../../fixtures/stress/issue15-cases.json"),
        "fixtures/golden/issue15-stress.json",
        include_str!("../../../fixtures/golden/issue15-stress.json"),
    ),
    (
        "stress",
        "fixtures/stress/issue16-cases.json",
        include_str!("../../../fixtures/stress/issue16-cases.json"),
        "fixtures/golden/issue16-stress.json",
        include_str!("../../../fixtures/golden/issue16-stress.json"),
    ),
    (
        "stress",
        "fixtures/stress/issue18-cases.json",
        include_str!("../../../fixtures/stress/issue18-cases.json"),
        "fixtures/golden/issue18-stress.json",
        include_str!("../../../fixtures/golden/issue18-stress.json"),
    ),
    (
        "stress",
        "fixtures/stress/issue19-cases.json",
        include_str!("../../../fixtures/stress/issue19-cases.json"),
        "fixtures/golden/issue19-stress.json",
        include_str!("../../../fixtures/golden/issue19-stress.json"),
    ),
    (
        "stress",
        "fixtures/stress/issue20-cases.json",
        include_str!("../../../fixtures/stress/issue20-cases.json"),
        "fixtures/golden/issue20-stress.json",
        include_str!("../../../fixtures/golden/issue20-stress.json"),
    ),
];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fixture {
    pub id: String,
    pub rule: String,
    pub description: String,
    pub input: Input,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Input {
    Evolution {
        scenario: Box<super::selfcheck_policy::EvolutionFixture>,
    },
    Manual {
        record: Box<crate::domain::ManualRecord>,
        expected: Box<crate::domain::ManualSubject>,
        now: u64,
        #[serde(default)]
        tamper: bool,
    },
    Compatibility {
        content: String,
        classes: BTreeSet<String>,
    },
    Snapshot {
        scenario: SnapshotScenario,
    },
    Runner {
        scenario: RunnerScenario,
    },
    Rule {
        #[serde(default)]
        parameters: BTreeMap<String, Value>,
        #[serde(default)]
        files: BTreeMap<String, String>,
        #[serde(default)]
        binary_files: BTreeMap<String, Vec<u8>>,
        #[serde(default)]
        base_files: BTreeMap<String, String>,
        #[serde(default)]
        commits: Vec<String>,
        #[serde(default)]
        projects: Vec<ProjectFacts>,
        #[serde(default)]
        custom: Option<Box<CustomRule>>,
    },
    Report {
        format: ReportFormat,
        content: String,
    },
    Policy {
        content: String,
    },
    Gate {
        checks: Vec<CheckResult>,
        required: Vec<String>,
        invalid: Vec<String>,
    },
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotScenario {
    Normal,
    Staged,
    Oversized,
    Symlink,
    MissingObject,
    LongPath,
    ChangedInput,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RunnerScenario {
    Success,
    Failure,
    Timeout,
    Overflow,
    MissingTool,
}

#[derive(Debug)]
pub struct Case {
    pub suite: String,
    pub fixture: Fixture,
    pub input: String,
    pub golden: BTreeMap<String, Value>,
}

pub fn load() -> Result<Vec<Case>> {
    let mut cases = Vec::new();
    for (suite, path, input, golden_path, golden) in SUITES {
        cases.extend(parse_suite(suite, path, input, golden_path, golden)?);
    }
    Ok(cases)
}

fn parse_suite(
    suite: &str,
    path: &str,
    input: &str,
    golden_path: &str,
    golden: &str,
) -> Result<Vec<Case>> {
    if input.len() + golden.len() > 2 * 1024 * 1024 {
        bail!("Fixture suite {suite} exceeds 2 MiB");
    }
    let fixtures: Vec<Fixture> =
        serde_json::from_str(input).with_context(|| format!("Invalid {path}"))?;
    // Validate mapping uniqueness before typed maps can overwrite keys.
    let _: serde_norway::Value =
        serde_norway::from_str(golden).with_context(|| format!("Invalid {golden_path} mapping"))?;
    let mut expected: BTreeMap<String, BTreeMap<String, Value>> =
        serde_json::from_str(golden).with_context(|| format!("Invalid {golden_path}"))?;
    if fixtures.is_empty() || fixtures.len() > 512 {
        bail!("Suite {suite} requires 1..512 fixtures");
    }
    let mut ids = BTreeSet::new();
    let mut cases = Vec::new();
    for (index, fixture) in fixtures.into_iter().enumerate() {
        if fixture.id.is_empty()
            || fixture.rule.is_empty()
            || fixture.description.is_empty()
            || !ids.insert(fixture.id.clone())
        {
            bail!("Invalid or duplicate fixture {suite}/{}", fixture.id);
        }
        let golden = expected
            .remove(&fixture.id)
            .with_context(|| format!("Missing golden for {suite}/{}", fixture.id))?;
        if golden.is_empty()
            || golden.keys().any(|pointer| !pointer.starts_with('/'))
            || golden.values().any(|value| {
                value.get("$contains").is_some()
                    && (value.as_object().is_none_or(|object| object.len() != 1)
                        || value["$contains"].as_str().is_none_or(str::is_empty))
            })
        {
            bail!("Empty or invalid assertions for {suite}/{}", fixture.id);
        }
        cases.push(Case {
            suite: (*suite).into(),
            fixture,
            input: format!("{path}#/{index}/input"),
            golden,
        });
    }
    if !expected.is_empty() {
        bail!(
            "Orphan goldens in {suite}: {:?}",
            expected.keys().collect::<Vec<_>>()
        );
    }
    Ok(cases)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn incomplete_fixture_inventory_is_rejected() {
        let case =
            r#"[{"id":"a","rule":"line-ending","description":"case","input":{"kind":"rule"}}]"#;
        for (input, golden) in [
            ("[]", "{}"),
            ("invalid", "{}"),
            (case, "{}"),
            (case, r#"{"a":{}}"#),
            (case, r#"{"a":{"bad":1}}"#),
            (case, r#"{"a":{"/verdict":"pass"},"b":{}}"#),
        ] {
            assert!(
                parse_suite(
                    "test",
                    "fixtures/test/cases.json",
                    input,
                    "fixtures/golden/test.json",
                    golden
                )
                .is_err()
            );
        }
        assert!(
            parse_suite(
                "test",
                "fixtures/test/cases.json",
                case,
                "fixtures/golden/test.json",
                r#"{"a":{"/verdict":"pass"}}"#
            )
            .is_ok()
        );
        for golden in [
            r#"{"a":{"/verdict":"pass"},"a":{"/verdict":"fail"}}"#,
            r#"{"a":{"/verdict":"pass","/verdict":"fail"}}"#,
            r#"{"a":{"/reason":{"$contains":""}}}"#,
            r#"{"a":{"/reason":{"$contains":"text","ignored":true}}}"#,
        ] {
            assert!(
                parse_suite(
                    "test",
                    "fixtures/test/cases.json",
                    case,
                    "fixtures/golden/test.json",
                    golden
                )
                .is_err()
            );
        }
    }
}
