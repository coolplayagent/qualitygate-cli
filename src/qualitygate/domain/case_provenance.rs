//! Declared case history is evidence, never an authentication of independence.
use super::evolution::{Actor, ActorKind, valid_digest, validate_text};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaseOrigin {
    Generated,
    Observed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Classification {
    Generated,
    HumanConfirmed,
    Independent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseProvenance {
    pub case_id: String,
    pub origin: CaseOrigin,
    pub classification: Classification,
    pub reviewer: Option<Actor>,
    pub used_for_tuning: bool,
    pub derived_from: Vec<String>,
    pub expected_detection: String,
    pub expected_allowed: String,
}

impl CaseProvenance {
    pub fn validate(&self) -> Result<(), String> {
        validate_text(&self.case_id, 256)?;
        validate_text(&self.expected_detection, 4096)?;
        validate_text(&self.expected_allowed, 4096)?;
        if self.derived_from.len() > 32
            || self.derived_from.iter().collect::<BTreeSet<_>>().len() != self.derived_from.len()
            || self.derived_from.iter().any(|value| !valid_digest(value))
        {
            return Err("Case ancestry requires at most 32 distinct evidence digests".into());
        }
        if let Some(reviewer) = &self.reviewer {
            reviewer.validate()?;
        }
        if self.classification != Classification::Generated
            && !self
                .reviewer
                .as_ref()
                .is_some_and(|actor| actor.kind == ActorKind::Human)
        {
            return Err(
                "Confirmed and independent cases require an explicit human reviewer".into(),
            );
        }
        if self.classification == Classification::Generated && self.origin != CaseOrigin::Generated
        {
            return Err("Generated classification requires generated origin".into());
        }
        if self.classification == Classification::Independent && self.contaminated() {
            return Err("Generated or tuning cases cannot be independent".into());
        }
        Ok(())
    }

    pub fn contaminated(&self) -> bool {
        self.origin == CaseOrigin::Generated || self.used_for_tuning
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn origin_review_and_tuning_are_independent_obligations() {
        let mut case = CaseProvenance {
            case_id: "case".into(),
            origin: CaseOrigin::Observed,
            classification: Classification::Independent,
            reviewer: Some(Actor {
                id: "reviewer".into(),
                kind: ActorKind::Human,
            }),
            used_for_tuning: false,
            derived_from: vec![],
            expected_detection: "bug".into(),
            expected_allowed: "normal".into(),
        };
        assert!(case.validate().is_ok());
        case.used_for_tuning = true;
        assert!(case.validate().is_err());
        case.used_for_tuning = false;
        case.origin = CaseOrigin::Generated;
        assert!(case.validate().is_err());
        case.classification = Classification::HumanConfirmed;
        assert!(case.validate().is_ok());
        case.reviewer.as_mut().unwrap().kind = ActorKind::Agent;
        assert!(case.validate().is_err());
        case.classification = Classification::Generated;
        assert!(case.validate().is_ok());
        case.derived_from = vec!["invalid".into()];
        assert!(case.validate().is_err());
        case.derived_from = vec![format!("sha256:{}", "a".repeat(64)); 2];
        assert!(case.validate().is_err());
        case.derived_from.clear();
        case.origin = CaseOrigin::Observed;
        assert!(case.validate().is_err());
        case.origin = CaseOrigin::Generated;
        case.expected_allowed.clear();
        assert!(case.validate().is_err());
    }
}
