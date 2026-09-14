//! Diagnostic debt cannot grow within any tool/rule bucket.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Key {
    pub tool: Option<String>,
    pub rule: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Measurement {
    pub key: Key,
    pub baseline: usize,
    pub current: usize,
}

impl Measurement {
    pub fn increased(&self) -> bool {
        self.current > self.baseline
    }
}

/// Counts include multiplicity. Independent buckets cannot offset each other.
pub fn compare(
    baseline: &BTreeMap<Key, usize>,
    current: &BTreeMap<Key, usize>,
) -> Vec<Measurement> {
    baseline
        .keys()
        .chain(current.keys())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|key| Measurement {
            key: key.clone(),
            baseline: baseline.get(key).copied().unwrap_or(0),
            current: current.get(key).copied().unwrap_or(0),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debt_decreases_never_hide_growth_in_another_rule_or_tool() {
        let key = |tool, rule: &str| Key {
            tool,
            rule: rule.into(),
        };
        let a = key(None, "a");
        let b = key(None, "b");
        let other = key(Some("other".into()), "a");
        let previous = BTreeMap::from([(a.clone(), 10), (b.clone(), 2)]);
        let current = BTreeMap::from([(a.clone(), 1), (other.clone(), 1)]);
        let measurements = compare(&previous, &current);
        assert_eq!(measurements.len(), 3);
        assert_eq!(
            measurements
                .iter()
                .filter(|value| value.increased())
                .map(|value| &value.key)
                .collect::<Vec<_>>(),
            vec![&other]
        );
        assert_eq!(
            measurements[1],
            Measurement {
                key: b,
                baseline: 2,
                current: 0
            }
        );
        assert!(
            compare(&previous, &previous)
                .iter()
                .all(|value| !value.increased())
        );
        assert!(compare(&BTreeMap::new(), &BTreeMap::new()).is_empty());
    }
}
