//! Review records are assertions carried by a caller-selected policy version.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceReview {
    pub id: String,
    pub reviewer: String,
    pub reference: String,
    pub resolution: ReviewResolution,
    pub rationale: String,
    pub binding_digest: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewResolution {
    InitialMapping,
    RuleUpdated,
    RuleUnchanged,
}

impl SourceReview {
    pub fn validate(&self) -> Result<(), String> {
        for (value, maximum) in [
            (&self.id, 128),
            (&self.reviewer, 256),
            (&self.reference, 2048),
            (&self.rationale, 4096),
        ] {
            if value.trim().is_empty()
                || value.len() > maximum
                || value.chars().any(char::is_control)
            {
                return Err("Source review fields must be nonempty, bounded text without control characters".into());
            }
        }
        if !self
            .binding_digest
            .strip_prefix("sha256:")
            .is_some_and(|digest| {
                digest.len() == 64
                    && digest
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            })
        {
            return Err("Source review requires a lowercase sha256 binding_digest".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewStatus {
    Missing,
    Stale,
    Bound,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceReviewEvidence {
    pub expected_binding_digest: String,
    pub status: ReviewStatus,
    pub record: Option<SourceReview>,
}

impl SourceReviewEvidence {
    pub fn new(expected_binding_digest: String, record: Option<SourceReview>) -> Self {
        let status = match &record {
            None => ReviewStatus::Missing,
            Some(record)
                if record.validate().is_ok()
                    && record.binding_digest == expected_binding_digest =>
            {
                ReviewStatus::Bound
            }
            Some(_) => ReviewStatus::Stale,
        };
        Self {
            expected_binding_digest,
            status,
            record,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn review_binding_and_record_constraints_are_pure_and_explicit() {
        let digest = format!("sha256:{}", "1".repeat(64));
        let valid = SourceReview {
            id: "review-1".into(),
            reviewer: "team".into(),
            reference: "PR-42".into(),
            resolution: ReviewResolution::RuleUnchanged,
            rationale: "The executable assertions still match the clarified section".into(),
            binding_digest: digest.clone(),
        };
        assert_eq!(
            SourceReviewEvidence::new(digest.clone(), None).status,
            ReviewStatus::Missing
        );
        assert_eq!(
            SourceReviewEvidence::new(digest.clone(), Some(valid.clone())).status,
            ReviewStatus::Bound
        );
        for field in ["id", "reviewer", "reference", "rationale", "binding_digest"] {
            let mut value = serde_json::to_value(&valid).unwrap();
            value[field] = serde_json::json!("");
            let invalid: SourceReview = serde_json::from_value(value).unwrap();
            assert!(invalid.validate().is_err());
            assert_eq!(
                SourceReviewEvidence::new(digest.clone(), Some(invalid)).status,
                ReviewStatus::Stale
            );
        }
        for text in ["bad\nrecord".to_owned(), "x".repeat(129)] {
            let mut invalid = valid.clone();
            invalid.id = text;
            assert!(invalid.validate().is_err());
        }
        assert_eq!(
            SourceReviewEvidence::new(format!("sha256:{}", "2".repeat(64)), Some(valid)).status,
            ReviewStatus::Stale
        );
    }
}
