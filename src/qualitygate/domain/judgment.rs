//! Pure, optional assessment contracts. An assessment never changes a check gate.

use super::Artifact;
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}
fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

pub fn provider_binding(
    executable: &Artifact,
    version_digest: &str,
    inputs: &[Artifact],
) -> Result<String> {
    if !valid_digest(&executable.digest)
        || !valid_digest(version_digest)
        || inputs.iter().any(|input| !valid_digest(&input.digest))
    {
        bail!("Provider executable, version and input digests are required");
    }
    Ok(digest(&serde_json::to_vec(&(
        &executable.digest,
        version_digest,
        inputs,
    ))?))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JudgmentMode {
    Shadow,
    Advisory,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Question {
    pub id: String,
    pub version: u32,
    pub text: String,
    pub criteria: Vec<String>,
    pub choices: Vec<String>,
    pub positive_choice: String,
    pub target_event: String,
    pub source_digest: String,
}

impl Question {
    pub fn digest(&self) -> Result<String> {
        Ok(digest(&serde_json::to_vec(&(
            &self.id,
            self.version,
            &self.text,
            &self.criteria,
            &self.choices,
            &self.positive_choice,
            &self.target_event,
        ))?))
    }
    pub fn validate(&self) -> Result<()> {
        if self.id.trim().is_empty()
            || self.version == 0
            || self.text.trim().is_empty()
            || self.criteria.is_empty()
            || self.target_event.trim().is_empty()
        {
            bail!("Question id, version, text, criteria and target event are required");
        }
        if self.choices.len() < 2
            || self.choices.len() > 16
            || self.choices.iter().any(|item| item.trim().is_empty())
            || self.choices.iter().collect::<BTreeSet<_>>().len() != self.choices.len()
        {
            bail!("Question choices must be distinct, nonempty and bounded");
        }
        if !self.choices.contains(&self.positive_choice) {
            bail!("Question positive choice must be a declared choice");
        }
        if !valid_digest(&self.source_digest) || self.source_digest != self.digest()? {
            bail!("Question source digest does not bind its exact text, criteria and choices");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CalibrationStatus {
    Valid,
    Stale,
    InsufficientSamples,
    OutOfDomain,
    Revoked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RiskCoveragePoint {
    pub coverage: f64,
    pub risk: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CalibrationRecord {
    pub id: String,
    pub declared_status: CalibrationStatus,
    pub provider_digest: String,
    pub model_digest: String,
    pub calibrator_digest: String,
    pub label_source_digest: String,
    pub question_digest: String,
    pub target_event: String,
    pub repository_digest: String,
    pub language: String,
    pub rule_id: String,
    pub labeled_from_unix: u64,
    pub labeled_until_unix: u64,
    pub expires_unix: u64,
    pub revoked: bool,
    pub sample_count: usize,
    pub independent_label_count: usize,
    pub brier_score: f64,
    pub log_loss: f64,
    pub near_threshold_error: f64,
    pub risk_coverage: Vec<RiskCoveragePoint>,
    pub known_limits: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct EvaluationContext<'a> {
    pub evidence_id: &'a str,
    pub provider_digest: &'a str,
    pub repository_digest: &'a str,
    pub language: &'a str,
    pub rule_id: &'a str,
    pub now_unix: u64,
}

impl CalibrationRecord {
    pub fn status(
        &self,
        question: &Question,
        model_digest: &str,
        context: &EvaluationContext<'_>,
    ) -> CalibrationStatus {
        if self.revoked {
            return CalibrationStatus::Revoked;
        }
        if context.now_unix >= self.expires_unix
            || self.labeled_until_unix < self.labeled_from_unix
            || self.expires_unix <= self.labeled_until_unix
        {
            return CalibrationStatus::Stale;
        }
        if self.sample_count < 30
            || self.independent_label_count < 30
            || self.independent_label_count > self.sample_count
        {
            return CalibrationStatus::InsufficientSamples;
        }
        if self.provider_digest != context.provider_digest
            || self.model_digest != model_digest
            || self.question_digest != question.source_digest
            || self.target_event != question.target_event
            || self.repository_digest != context.repository_digest
            || self.language != context.language
            || self.rule_id != context.rule_id
        {
            return CalibrationStatus::OutOfDomain;
        }
        CalibrationStatus::Valid
    }
    pub fn validate_metrics(&self) -> Result<()> {
        if self.id.trim().is_empty()
            || ![
                &self.provider_digest,
                &self.model_digest,
                &self.calibrator_digest,
                &self.label_source_digest,
                &self.question_digest,
                &self.repository_digest,
            ]
            .into_iter()
            .all(|value| valid_digest(value))
        {
            bail!("Calibration identity or digests are invalid");
        }
        if !self.brier_score.is_finite()
            || !(0.0..=1.0).contains(&self.brier_score)
            || !self.log_loss.is_finite()
            || self.log_loss < 0.0
            || !self.near_threshold_error.is_finite()
            || !(0.0..=1.0).contains(&self.near_threshold_error)
        {
            bail!("Calibration scores must be finite and within their metric ranges");
        }
        if self.risk_coverage.is_empty()
            || self.risk_coverage.len() > 64
            || self.risk_coverage.iter().any(|point| {
                !point.coverage.is_finite()
                    || !(0.0..=1.0).contains(&point.coverage)
                    || !point.risk.is_finite()
                    || !(0.0..=1.0).contains(&point.risk)
            })
        {
            bail!("Calibration risk-coverage points must be finite and bounded");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JudgmentPolicy {
    pub schema_version: u32,
    pub mode: JudgmentMode,
    pub argv: Vec<String>,
    pub version_argv: Vec<String>,
    pub provider_inputs: Vec<String>,
    pub timeout_seconds: u32,
    pub question: Question,
    pub calibration: Option<CalibrationRecord>,
    pub review_priority_threshold: Option<f64>,
}

impl JudgmentPolicy {
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != 1
            || self.argv.is_empty()
            || self.argv.len() > 32
            || self.version_argv.is_empty()
            || self.version_argv.len() > 32
            || self.provider_inputs.len() > 16
            || self.argv[0] != self.version_argv[0]
            || self
                .argv
                .iter()
                .chain(&self.version_argv)
                .any(|arg| arg.is_empty() || arg.len() > 4096)
            || self
                .provider_inputs
                .iter()
                .any(|path| path.is_empty() || path.len() > 4096)
            || !(1..=120).contains(&self.timeout_seconds)
        {
            bail!("Judgment policy has invalid version, argv or timeout");
        }
        self.question.validate()?;
        if let Some(calibration) = &self.calibration {
            calibration.validate_metrics()?;
        }
        if self
            .review_priority_threshold
            .is_some_and(|value| !value.is_finite() || !(0.0..=1.0).contains(&value))
        {
            bail!("Review priority threshold must be a finite probability");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbabilityPrimitive {
    Boolean,
    Choice,
    Ordinal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Applicability {
    InDomain,
    OutOfDomain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AbstentionReason {
    InsufficientEvidence,
    Ambiguous,
    OutOfDomain,
    PolicyUndefined,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Assessment {
    Deterministic {
        question_id: String,
        value: bool,
        evidence_refs: Vec<String>,
    },
    Probabilistic {
        question_id: String,
        primitive: ProbabilityPrimitive,
        selected: String,
        distribution: BTreeMap<String, f64>,
        target_event: String,
        applicability: Applicability,
        calibration_ref: String,
        model_digest: String,
        evidence_refs: Vec<String>,
    },
    Abstained {
        question_id: String,
        reason: AbstentionReason,
        evidence_refs: Vec<String>,
    },
    ExecutionGap {
        reason: String,
    },
}

impl Assessment {
    pub fn validate(
        &self,
        policy: &JudgmentPolicy,
        context: &EvaluationContext<'_>,
    ) -> Result<Option<CalibrationStatus>> {
        match self {
            Self::ExecutionGap { reason } => {
                if reason.trim().is_empty() {
                    bail!("Execution gap requires a reason");
                }
                Ok(None)
            }
            Self::Deterministic {
                question_id,
                evidence_refs,
                ..
            } => {
                if policy.question.choices.len() != 2
                    || question_id != &policy.question.id
                    || evidence_refs != &[context.evidence_id]
                {
                    bail!("Boolean assessment requires a binary question and bound evidence");
                }
                Ok(None)
            }
            Self::Abstained {
                question_id,
                evidence_refs,
                ..
            } => {
                if question_id != &policy.question.id || evidence_refs != &[context.evidence_id] {
                    bail!("Assessment question or evidence binding differs from the request");
                }
                Ok(None)
            }
            Self::Probabilistic {
                question_id,
                primitive,
                selected,
                distribution,
                target_event,
                applicability,
                calibration_ref,
                model_digest,
                evidence_refs,
            } => {
                if question_id != &policy.question.id
                    || target_event != &policy.question.target_event
                    || evidence_refs != &[context.evidence_id]
                    || distribution.len() != policy.question.choices.len()
                    || !distribution.keys().eq(policy
                        .question
                        .choices
                        .iter()
                        .collect::<BTreeSet<_>>())
                    || !distribution.contains_key(selected)
                {
                    bail!("Probability output changes the question, choices or evidence binding");
                }
                if *primitive == ProbabilityPrimitive::Boolean
                    && policy.question.choices != ["true", "false"]
                {
                    bail!("Boolean probability requires true/false choices");
                }
                if distribution
                    .values()
                    .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
                    || (distribution.values().sum::<f64>() - 1.0).abs() > 0.000_001
                {
                    bail!("Probability distribution is invalid or not normalized");
                }
                let selected_probability = distribution[selected];
                if distribution
                    .values()
                    .any(|probability| *probability > selected_probability)
                {
                    bail!("Selected choice must have maximal probability");
                }
                if *applicability == Applicability::OutOfDomain {
                    bail!("Out-of-domain probability must abstain");
                }
                let calibration = policy
                    .calibration
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("Probability requires a calibration record"))?;
                if calibration_ref != &calibration.id {
                    bail!("Probability calibration reference differs from policy");
                }
                let status = calibration.status(&policy.question, model_digest, context);
                if status != CalibrationStatus::Valid || calibration.declared_status != status {
                    bail!(
                        "Calibration is not valid for this provider, model, question, repository, rule and time: {status:?}"
                    );
                }
                Ok(Some(status))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn question() -> Question {
        let mut question = Question {
            id: "qg.review".into(),
            version: 1,
            text: "Is the finding valid?".into(),
            criteria: vec!["Independent review".into()],
            choices: vec!["true".into(), "false".into()],
            positive_choice: "true".into(),
            target_event: "finding_valid".into(),
            source_digest: String::new(),
        };
        question.source_digest = question.digest().unwrap();
        question
    }

    fn calibration(question: &Question) -> CalibrationRecord {
        CalibrationRecord {
            id: "cal".into(),
            declared_status: CalibrationStatus::Valid,
            provider_digest: digest(b"provider"),
            model_digest: digest(b"model"),
            calibrator_digest: digest(b"calibrator"),
            label_source_digest: digest(b"independent-labels"),
            question_digest: question.source_digest.clone(),
            target_event: question.target_event.clone(),
            repository_digest: digest(b"repo"),
            language: "rust".into(),
            rule_id: "warning".into(),
            labeled_from_unix: 100,
            labeled_until_unix: 200,
            expires_unix: 500,
            revoked: false,
            sample_count: 40,
            independent_label_count: 40,
            brier_score: 0.1,
            log_loss: 0.2,
            near_threshold_error: 0.05,
            risk_coverage: vec![RiskCoveragePoint {
                coverage: 0.8,
                risk: 0.1,
            }],
            known_limits: vec![],
        }
    }

    #[test]
    fn question_bytes_and_choices_are_versioned_by_source_digest() {
        let mut q = question();
        q.validate().unwrap();
        q.text.push_str(" New criterion");
        assert!(q.validate().is_err());
        q.source_digest = q.digest().unwrap();
        q.validate().unwrap();
        q.choices.push("unknown".into());
        assert!(q.validate().is_err());
    }

    #[test]
    fn calibration_scope_expiry_revocation_and_labels_fail_closed() {
        let q = question();
        let mut c = calibration(&q);
        c.validate_metrics().unwrap();
        let status = |record: &CalibrationRecord, repo: &str, time| {
            record.status(
                &q,
                &digest(b"model"),
                &EvaluationContext {
                    evidence_id: "evidence",
                    provider_digest: &digest(b"provider"),
                    repository_digest: repo,
                    language: "rust",
                    rule_id: "warning",
                    now_unix: time,
                },
            )
        };
        assert_eq!(status(&c, &digest(b"repo"), 300), CalibrationStatus::Valid);
        assert_eq!(
            status(&c, &digest(b"other"), 300),
            CalibrationStatus::OutOfDomain
        );
        assert_eq!(status(&c, &digest(b"repo"), 500), CalibrationStatus::Stale);
        assert_eq!(
            c.status(
                &q,
                &digest(b"changed-model"),
                &EvaluationContext {
                    evidence_id: "evidence",
                    provider_digest: &digest(b"provider"),
                    repository_digest: &digest(b"repo"),
                    language: "rust",
                    rule_id: "warning",
                    now_unix: 300,
                }
            ),
            CalibrationStatus::OutOfDomain
        );
        let mut changed_question = q.clone();
        changed_question.text.push_str(" Changed");
        changed_question.source_digest = changed_question.digest().unwrap();
        assert_eq!(
            c.status(
                &changed_question,
                &digest(b"model"),
                &EvaluationContext {
                    evidence_id: "evidence",
                    provider_digest: &digest(b"provider"),
                    repository_digest: &digest(b"repo"),
                    language: "rust",
                    rule_id: "warning",
                    now_unix: 300,
                }
            ),
            CalibrationStatus::OutOfDomain
        );
        c.sample_count = 10;
        assert_eq!(
            status(&c, &digest(b"repo"), 300),
            CalibrationStatus::InsufficientSamples
        );
        c.revoked = true;
        assert_eq!(
            status(&c, &digest(b"repo"), 300),
            CalibrationStatus::Revoked
        );
    }

    #[test]
    fn assessment_union_rejects_invalid_distribution_and_unknown_variant() {
        let q = question();
        let policy = JudgmentPolicy {
            schema_version: 1,
            mode: JudgmentMode::Shadow,
            argv: vec!["provider".into()],
            version_argv: vec!["provider".into(), "--version".into()],
            provider_inputs: vec![],
            timeout_seconds: 5,
            question: q.clone(),
            calibration: Some(calibration(&q)),
            review_priority_threshold: Some(0.7),
        };
        policy.validate().unwrap();
        let valid = json!({"kind":"probabilistic","question_id":"qg.review","primitive":"boolean","selected":"true","distribution":{"true":0.8,"false":0.2},"target_event":"finding_valid","applicability":"in_domain","calibration_ref":"cal","model_digest":digest(b"model"),"evidence_refs":["evidence"]});
        let mut unsupported_confidence = valid.clone();
        unsupported_confidence["confidence"] = json!(0.9);
        assert!(serde_json::from_value::<Assessment>(unsupported_confidence).is_err());
        let assessment: Assessment = serde_json::from_value(valid.clone()).unwrap();
        let context = EvaluationContext {
            evidence_id: "evidence",
            provider_digest: &digest(b"provider"),
            repository_digest: &digest(b"repo"),
            language: "rust",
            rule_id: "warning",
            now_unix: 300,
        };
        assessment.validate(&policy, &context).unwrap();
        let mut nonbinary = policy.clone();
        nonbinary.question.choices.push("unknown".into());
        nonbinary.question.source_digest = nonbinary.question.digest().unwrap();
        let deterministic = Assessment::Deterministic {
            question_id: q.id.clone(),
            value: true,
            evidence_refs: vec!["evidence".into()],
        };
        assert!(deterministic.validate(&nonbinary, &context).is_err());
        for corrupted in [
            json!({"kind":"unknown"}),
            json!({"kind":"probabilistic","question_id":"qg.review","primitive":"boolean","selected":"true","distribution":{"true":0.8,"false":0.3},"target_event":"finding_valid","applicability":"in_domain","calibration_ref":"cal","model_digest":digest(b"model"),"evidence_refs":["evidence"]}),
            json!({"kind":"probabilistic","question_id":"qg.review","primitive":"boolean","selected":"false","distribution":{"true":0.8,"false":0.2},"target_event":"finding_valid","applicability":"in_domain","calibration_ref":"cal","model_digest":digest(b"model"),"evidence_refs":["evidence"]}),
        ] {
            if let Ok(parsed) = serde_json::from_value::<Assessment>(corrupted) {
                assert!(parsed.validate(&policy, &context).is_err());
            }
        }
    }
}
