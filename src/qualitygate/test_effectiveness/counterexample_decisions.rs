//! Pure decisions over producer-attributed test cases in two immutable inputs.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum TestOutcome {
    Passed,
    Failure { types: Vec<String> },
    Error,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestCase {
    pub file: String,
    pub classname: String,
    pub name: String,
    pub outcome: TestOutcome,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileProof {
    pub file: String,
    pub executed: usize,
    pub counterexamples: usize,
}

type Identity<'a> = (&'a str, &'a str, &'a str);

fn inventory<'a>(
    cases: &'a [TestCase],
    selected: &BTreeSet<String>,
    allowed: &[String],
) -> Result<BTreeMap<Identity<'a>, &'a TestCase>, String> {
    let mut identities = BTreeSet::new();
    let mut matching = BTreeMap::new();
    let mut covered = BTreeSet::new();
    for case in cases {
        let identity = (
            case.file.as_str(),
            case.classname.as_str(),
            case.name.as_str(),
        );
        if case.file.is_empty() || case.name.trim().is_empty() || !identities.insert(identity) {
            return Err("Test report contains missing or duplicate case identities".into());
        }
        match &case.outcome {
            TestOutcome::Error => return Err("Test report contains an execution error".into()),
            TestOutcome::Failure { types }
                if types.is_empty() || types.iter().any(|kind| !allowed.contains(kind)) =>
            {
                return Err("Test failure has an unknown or missing assertion type".into());
            }
            TestOutcome::Skipped if selected.contains(&case.file) => {
                return Err("Selected test case was skipped".into());
            }
            _ => {}
        }
        if selected.contains(&case.file) {
            covered.insert(&case.file);
            matching.insert(identity, case);
        }
    }
    for file in selected {
        if !covered.contains(file) {
            return Err(format!("Selected test file was not executed: {file}"));
        }
    }
    Ok(matching)
}

pub fn current_passes(
    cases: &[TestCase],
    selected: &BTreeSet<String>,
    allowed: &[String],
) -> Result<bool, String> {
    inventory(cases, selected, allowed)?;
    Ok(!cases
        .iter()
        .any(|case| matches!(case.outcome, TestOutcome::Failure { .. })))
}

pub fn compare(
    current: &[TestCase],
    baseline: &[TestCase],
    selected: &BTreeSet<String>,
    allowed: &[String],
) -> Result<Vec<FileProof>, String> {
    if selected.is_empty() {
        return Err("No test files selected for effectiveness proof".into());
    }
    if !current_passes(current, selected, allowed)? {
        return Err("Current tests did not pass".into());
    }
    let current = inventory(current, selected, allowed)?;
    let baseline = inventory(baseline, selected, allowed)?;
    if current.keys().ne(baseline.keys()) {
        return Err("Selected test case identities differ between snapshots".into());
    }
    let mut proof: BTreeMap<_, _> = selected
        .iter()
        .map(|file| {
            (
                file.as_str(),
                FileProof {
                    file: file.clone(),
                    executed: 0,
                    counterexamples: 0,
                },
            )
        })
        .collect();
    for case in baseline.values() {
        let file = proof
            .get_mut(case.file.as_str())
            .expect("inventory only contains selected files");
        file.executed += 1;
        file.counterexamples += usize::from(matches!(case.outcome, TestOutcome::Failure { .. }));
    }
    Ok(proof.into_values().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn case(file: &str, outcome: TestOutcome) -> TestCase {
        TestCase {
            file: file.into(),
            classname: "Suite".into(),
            name: "behavior".into(),
            outcome,
        }
    }
    #[test]
    fn each_file_needs_its_own_counterexample_and_complete_comparable_cases() {
        let allowed = vec!["AssertionError".into()];
        let selected = BTreeSet::from(["a.rs".into(), "b.rs".into()]);
        let current = vec![
            case("a.rs", TestOutcome::Passed),
            case("b.rs", TestOutcome::Passed),
        ];
        let mut base = current.clone();
        base[0].outcome = TestOutcome::Failure {
            types: allowed.clone(),
        };
        let proof = compare(&current, &base, &selected, &allowed).unwrap();
        assert_eq!((proof[0].counterexamples, proof[1].counterexamples), (1, 0));
        base[1].outcome = base[0].outcome.clone();
        assert!(
            compare(&current, &base, &selected, &allowed)
                .unwrap()
                .iter()
                .all(|file| file.counterexamples == 1)
        );
        for outcome in [
            TestOutcome::Error,
            TestOutcome::Skipped,
            TestOutcome::Failure { types: vec![] },
            TestOutcome::Failure {
                types: vec!["MissingSymbol".into()],
            },
        ] {
            base[0].outcome = outcome;
            assert!(compare(&current, &base, &selected, &allowed).is_err());
        }
        assert!(compare(&current, &current[..1], &selected, &allowed).is_err());
        let mut duplicate = current.clone();
        duplicate.push(current[0].clone());
        assert!(current_passes(&duplicate, &selected, &allowed).is_err());
        base = current.clone();
        base[0].name = "different".into();
        assert!(compare(&current, &base, &selected, &allowed).is_err());
        base[0].outcome = TestOutcome::Failure {
            types: allowed.clone(),
        };
        assert!(!current_passes(&base, &selected, &allowed).unwrap());
        assert!(compare(&base, &base, &selected, &allowed).is_err());
        assert!(compare(&current, &current, &BTreeSet::new(), &allowed).is_err());
        base[0].file.clear();
        assert!(current_passes(&base, &selected, &allowed).is_err());
    }
}
