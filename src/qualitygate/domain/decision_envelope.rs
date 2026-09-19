//! Versioned, typed command decisions. Construction and validation have no I/O.

use super::{
    Artifact, Decision, ExecutionStatus, Report, RuleValidationReport, VerificationBoundary,
    selfcheck::SelfcheckReport,
};
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

pub const SCHEMA_URI: &str = "urn:qualitygate:decision:1";
pub const PROTOCOL_VERSION: &str = "1.0.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandKind {
    Check,
    RuleValidation,
    Selfcheck,
    Feedback,
    PolicyTransition,
    Pilot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Route {
    Accept,
    Review,
    Block,
    InspectGap,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionSubject {
    pub snapshot_digest: Option<String>,
    pub task_digest: Option<String>,
    pub source_digest: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionPolicy {
    pub reference: Option<String>,
    pub digest: Option<String>,
    pub trust: String,
    pub evaluator_digest: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionExecution {
    pub complete: bool,
    pub omissions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionGate {
    pub outcome: Decision,
    pub route: Route,
    pub warning_count: usize,
    pub execution_gap_count: usize,
    pub pending_check_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionEnvelope {
    pub schema_uri: String,
    pub protocol_version: String,
    pub decision_id: String,
    pub command_kind: CommandKind,
    pub subject: DecisionSubject,
    pub policy: DecisionPolicy,
    pub execution: DecisionExecution,
    pub gate: DecisionGate,
    pub evidence_refs: Vec<Artifact>,
    pub verification: VerificationBoundary,
    pub payload: Value,
}

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

fn identity(envelope: &DecisionEnvelope) -> Result<String> {
    let mut bound = envelope.clone();
    bound.decision_id.clear();
    Ok(digest(&serde_json::to_vec(&bound)?))
}

fn route(
    decision: Decision,
    warning_count: usize,
    pending_count: usize,
    gap_count: usize,
) -> Route {
    match decision {
        Decision::Incomplete => Route::InspectGap,
        Decision::Fail => Route::Block,
        Decision::Pass if warning_count > 0 || pending_count > 0 || gap_count > 0 => Route::Review,
        Decision::Pass => Route::Accept,
    }
}

fn metadata_string(payload: &Value, key: &str) -> Result<Option<String>> {
    match payload.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        _ => bail!("Metadata field {key} must be a string when present"),
    }
}

impl DecisionEnvelope {
    pub fn from_metadata(kind: CommandKind, payload: Value, code: u8) -> Result<Self> {
        if !matches!(kind, CommandKind::PolicyTransition | CommandKind::Pilot)
            || !payload.is_object()
        {
            bail!("Metadata envelope requires a policy transition or pilot object");
        }
        let outcome = match code {
            0 => Decision::Pass,
            1 => Decision::Fail,
            2 => Decision::Incomplete,
            _ => bail!("Unknown command exit status"),
        };
        let complete = outcome != Decision::Incomplete;
        let source_digest = digest(&serde_json::to_vec(&payload)?);
        let evidence_refs = payload
            .get("artifacts")
            .map(|value| serde_json::from_value::<Vec<Artifact>>(value.clone()))
            .transpose()?
            .unwrap_or_default();
        let omissions = payload
            .get("omissions")
            .map(|value| serde_json::from_value::<Vec<String>>(value.clone()))
            .transpose()?
            .unwrap_or_default();
        let snapshot_digest = metadata_string(&payload, "snapshot_digest")?;
        let task_digest = metadata_string(&payload, "task_digest")?;
        let policy_ref = metadata_string(&payload, "policy_ref")?;
        let policy_digest = metadata_string(&payload, "policy_digest")?;
        let evaluator_digest = metadata_string(&payload, "evaluator_digest")?;
        let trust =
            metadata_string(&payload, "review_trust")?.unwrap_or_else(|| "local_candidate".into());
        let mut envelope = Self {
            schema_uri: SCHEMA_URI.into(),
            protocol_version: PROTOCOL_VERSION.into(),
            decision_id: String::new(),
            command_kind: kind,
            subject: DecisionSubject {
                snapshot_digest,
                task_digest,
                source_digest: Some(source_digest),
            },
            policy: DecisionPolicy {
                reference: policy_ref,
                digest: policy_digest,
                trust,
                evaluator_digest,
            },
            execution: DecisionExecution { complete, omissions },
            gate: DecisionGate {
                outcome,
                route: route(outcome, 0, 0, usize::from(!complete)),
                warning_count: 0,
                execution_gap_count: usize::from(!complete),
                pending_check_count: 0,
            },
            evidence_refs,
            verification: VerificationBoundary {
                conclusion: VerificationBoundary::conclusion(outcome).into(),
                verified_shapes: Vec::new(),
                known_limits: vec!["This command's outcome is interpreted within its command kind; it is not a repository check gate.".into()],
                unverified_assumptions: Vec::new(),
            },
            payload,
        };
        envelope.decision_id = identity(&envelope)?;
        envelope.validate()?;
        Ok(envelope)
    }
    pub fn from_check(report: &Report) -> Result<Self> {
        let warning_count = report
            .checks
            .iter()
            .filter(|check| check.severity == super::Severity::Warning)
            .map(|check| check.diagnostics.len())
            .sum();
        let execution_gap_count = report
            .checks
            .iter()
            .filter(|check| {
                check.execution.status != ExecutionStatus::Completed
                    && check.verdict != Some(super::Verdict::Skipped)
            })
            .count();
        let pending_check_count = report.plan.pending_delivery_checks.len();
        let payload = serde_json::to_value(report)?;
        let mut envelope = Self {
            schema_uri: SCHEMA_URI.into(),
            protocol_version: PROTOCOL_VERSION.into(),
            decision_id: String::new(),
            command_kind: CommandKind::Check,
            subject: DecisionSubject {
                snapshot_digest: Some(report.snapshot.content_digest.clone()),
                task_digest: report.policy.task_contract_digest.clone(),
                source_digest: None,
            },
            policy: DecisionPolicy {
                reference: Some(report.policy.source.clone()),
                digest: Some(report.policy.config_digest.clone()),
                trust: report.policy.trust.clone(),
                evaluator_digest: Some(report.evaluator_digest.clone()),
            },
            execution: DecisionExecution {
                complete: report.gate.complete,
                omissions: Vec::new(),
            },
            gate: DecisionGate {
                outcome: report.gate.decision,
                route: route(
                    report.gate.decision,
                    warning_count,
                    pending_check_count,
                    execution_gap_count,
                ),
                warning_count,
                execution_gap_count,
                pending_check_count,
            },
            evidence_refs: report
                .checks
                .iter()
                .flat_map(|check| check.execution.artifacts.iter().cloned())
                .collect(),
            verification: report.verification.clone(),
            payload,
        };
        envelope.decision_id = identity(&envelope)?;
        envelope.validate()?;
        Ok(envelope)
    }

    pub fn from_rule_validation(report: &RuleValidationReport) -> Result<Self> {
        let payload = serde_json::to_value(report)?;
        let mut envelope = Self {
            schema_uri: SCHEMA_URI.into(), protocol_version: PROTOCOL_VERSION.into(),
            decision_id: String::new(), command_kind: CommandKind::RuleValidation,
            subject: DecisionSubject { snapshot_digest: None, task_digest: None, source_digest: Some(digest(&serde_json::to_vec(&report.files)?)) },
            policy: DecisionPolicy { reference: Some(report.schema_id.clone()), digest: Some(report.schema_digest.clone()), trust: report.review_trust.clone(), evaluator_digest: None },
            execution: DecisionExecution { complete: report.complete, omissions: Vec::new() },
            gate: DecisionGate { outcome: report.decision, route: route(report.decision, 0, 0, usize::from(!report.complete)), warning_count: 0, execution_gap_count: usize::from(!report.complete), pending_check_count: 0 },
            evidence_refs: Vec::new(),
            verification: VerificationBoundary { conclusion: VerificationBoundary::conclusion(report.decision).into(), verified_shapes: vec![format!("{} rule files validated", report.files.len())], known_limits: vec!["Rule validation checks declarations and local bindings; it does not execute the proposed rule on a project.".into()], unverified_assumptions: Vec::new() }, payload,
        };
        envelope.decision_id = identity(&envelope)?;
        envelope.validate()?;
        Ok(envelope)
    }

    pub fn from_selfcheck(report: &SelfcheckReport) -> Result<Self> {
        let payload = serde_json::to_value(report)?;
        let mut envelope = Self {
            schema_uri: SCHEMA_URI.into(),
            protocol_version: PROTOCOL_VERSION.into(),
            decision_id: String::new(),
            command_kind: CommandKind::Selfcheck,
            subject: DecisionSubject {
                snapshot_digest: None,
                task_digest: None,
                source_digest: Some(report.corpus_digest.clone()),
            },
            policy: DecisionPolicy {
                reference: Some("shipped_selfcheck_corpus".into()),
                digest: Some(report.rules_digest.clone()),
                trust: "bundled".into(),
                evaluator_digest: None,
            },
            execution: DecisionExecution {
                complete: report.complete,
                omissions: Vec::new(),
            },
            gate: DecisionGate {
                outcome: report.decision,
                route: route(report.decision, 0, 0, report.errors.len()),
                warning_count: 0,
                execution_gap_count: report.errors.len(),
                pending_check_count: 0,
            },
            evidence_refs: Vec::new(),
            verification: report.verification.clone(),
            payload,
        };
        envelope.decision_id = identity(&envelope)?;
        envelope.validate()?;
        Ok(envelope)
    }

    pub fn from_feedback(report: &Report, payload: Value) -> Result<Self> {
        if payload["kind"] != "agent_feedback"
            || payload["run_id"] != report.run_id
            || payload["gate"]["decision"] != serde_json::to_value(report.gate.decision)?
            || payload["gate"]["complete"] != report.gate.complete
        {
            bail!("Feedback does not match its full check report");
        }
        let artifact: Artifact = serde_json::from_value(payload["full_report"].clone())?;
        let mut envelope = Self::from_check(report)?;
        envelope.command_kind = CommandKind::Feedback;
        envelope.evidence_refs = vec![artifact];
        envelope.execution.omissions = [
            "findings",
            "execution_gaps",
            "blockers",
            "pending_delivery_checks",
            "acceptance",
        ]
        .into_iter()
        .filter(|key| {
            payload[*key]["omitted"]
                .as_u64()
                .is_some_and(|count| count > 0)
        })
        .map(str::to_owned)
        .collect();
        envelope.payload = payload;
        envelope.decision_id = identity(&envelope)?;
        envelope.validate()?;
        Ok(envelope)
    }

    /// Preserve exact full-report pointers when a compact feedback view needs room
    /// for the envelope. Removed preview items are counted as omissions.
    pub fn compact_feedback(&mut self) -> Result<()> {
        if self.command_kind != CommandKind::Feedback {
            bail!("Only feedback previews can be compacted");
        }
        for key in [
            "findings",
            "execution_gaps",
            "blockers",
            "pending_delivery_checks",
            "acceptance",
        ] {
            let section = &mut self.payload[key];
            let retained = section["items"].as_array().map_or(0, Vec::len);
            if retained > 0 {
                section["items"] = serde_json::json!([]);
                section["omitted"] =
                    serde_json::json!(section["omitted"].as_u64().unwrap_or(0) + retained as u64);
                if !self.execution.omissions.iter().any(|item| item == key) {
                    self.execution.omissions.push(key.into());
                }
            }
        }
        self.payload["truncated"] = serde_json::json!(true);
        if !self.verification.verified_shapes.is_empty() {
            self.execution.omissions.push("verification_shapes".into());
            self.verification.verified_shapes.clear();
        }
        self.decision_id = identity(self)?;
        self.validate()
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema_uri != SCHEMA_URI || self.protocol_version != PROTOCOL_VERSION {
            bail!("Unknown decision schema or protocol version");
        }
        if [
            self.subject.snapshot_digest.as_deref(),
            self.subject.task_digest.as_deref(),
            self.subject.source_digest.as_deref(),
            self.policy.digest.as_deref(),
            self.policy.evaluator_digest.as_deref(),
        ]
        .into_iter()
        .flatten()
        .any(|value| !valid_digest(value))
            || self
                .evidence_refs
                .iter()
                .any(|artifact| artifact.path.is_empty() || !valid_digest(&artifact.digest))
        {
            bail!("Decision digest or evidence reference is invalid");
        }
        if self.decision_id != identity(self)? {
            bail!("Decision identity does not match envelope");
        }
        if self.execution.complete != (self.gate.outcome != Decision::Incomplete) {
            bail!("Incomplete execution cannot claim a complete decision");
        }
        if self.gate.route
            != route(
                self.gate.outcome,
                self.gate.warning_count,
                self.gate.pending_check_count,
                self.gate.execution_gap_count,
            )
        {
            bail!("Decision route conflicts with gate outcome or review signals");
        }
        match self.command_kind {
            CommandKind::Check => {
                let report: Report = serde_json::from_value(self.payload.clone())?;
                if report.gate.decision != self.gate.outcome
                    || report.gate.complete != self.execution.complete
                    || self.subject.snapshot_digest.as_deref()
                        != Some(&report.snapshot.content_digest)
                    || self.subject.task_digest != report.policy.task_contract_digest
                    || self.policy.reference.as_deref() != Some(&report.policy.source)
                    || self.policy.digest.as_deref() != Some(&report.policy.config_digest)
                    || self.policy.trust != report.policy.trust
                    || self.policy.evaluator_digest.as_deref() != Some(&report.evaluator_digest)
                    || !self.execution.omissions.is_empty()
                    || self.gate.warning_count
                        != report
                            .checks
                            .iter()
                            .filter(|check| check.severity == super::Severity::Warning)
                            .map(|check| check.diagnostics.len())
                            .sum::<usize>()
                    || self.gate.execution_gap_count
                        != report
                            .checks
                            .iter()
                            .filter(|check| {
                                check.execution.status != ExecutionStatus::Completed
                                    && check.verdict != Some(super::Verdict::Skipped)
                            })
                            .count()
                    || self.gate.pending_check_count != report.plan.pending_delivery_checks.len()
                    || serde_json::to_value(&self.evidence_refs)?
                        != serde_json::to_value(
                            report
                                .checks
                                .iter()
                                .flat_map(|check| check.execution.artifacts.iter())
                                .collect::<Vec<_>>(),
                        )?
                    || serde_json::to_value(&self.verification)?
                        != serde_json::to_value(&report.verification)?
                {
                    bail!("Check payload conflicts with envelope identity or outcome");
                }
            }
            CommandKind::RuleValidation => {
                let report: RuleValidationReport = serde_json::from_value(self.payload.clone())?;
                if report.decision != self.gate.outcome
                    || report.complete != self.execution.complete
                    || self.subject.source_digest.as_deref()
                        != Some(&digest(&serde_json::to_vec(&report.files)?))
                    || self.policy.reference.as_deref() != Some(&report.schema_id)
                    || self.policy.digest.as_deref() != Some(&report.schema_digest)
                    || self.policy.trust != report.review_trust
                    || self.gate.execution_gap_count != usize::from(!report.complete)
                {
                    bail!("Rule validation payload conflicts with envelope");
                }
            }
            CommandKind::Selfcheck => {
                let report: SelfcheckReport = serde_json::from_value(self.payload.clone())?;
                if report.decision != self.gate.outcome
                    || report.complete != self.execution.complete
                    || self.subject.source_digest.as_deref() != Some(&report.corpus_digest)
                    || self.policy.digest.as_deref() != Some(&report.rules_digest)
                    || self.gate.execution_gap_count != report.errors.len()
                {
                    bail!("Selfcheck payload conflicts with envelope");
                }
            }
            CommandKind::Feedback => {
                if self.payload["kind"] != "agent_feedback"
                    || self.payload["gate"]["decision"] != serde_json::to_value(self.gate.outcome)?
                    || self.payload["gate"]["complete"] != self.execution.complete
                    || self.evidence_refs.len() != 1
                    || serde_json::to_value(&self.evidence_refs[0])? != self.payload["full_report"]
                    || self.subject.snapshot_digest.as_deref()
                        != self.payload["snapshot"]["content_digest"].as_str()
                {
                    bail!("Feedback payload conflicts with envelope identity or outcome");
                }
                for key in [
                    "findings",
                    "execution_gaps",
                    "blockers",
                    "pending_delivery_checks",
                    "acceptance",
                ] {
                    if self.payload[key]["omitted"].as_u64().is_none_or(|omitted| {
                        (omitted > 0) != self.execution.omissions.iter().any(|item| item == key)
                    }) {
                        bail!("Feedback omission count conflicts with envelope: {key}");
                    }
                }
            }
            CommandKind::PolicyTransition | CommandKind::Pilot => {
                if !self.payload.is_object()
                    || self.subject.source_digest.as_deref()
                        != Some(&digest(&serde_json::to_vec(&self.payload)?))
                    || self.subject.snapshot_digest
                        != metadata_string(&self.payload, "snapshot_digest")?
                    || self.subject.task_digest != metadata_string(&self.payload, "task_digest")?
                    || self.policy.reference != metadata_string(&self.payload, "policy_ref")?
                    || self.policy.digest != metadata_string(&self.payload, "policy_digest")?
                    || self.policy.evaluator_digest
                        != metadata_string(&self.payload, "evaluator_digest")?
                    || self.policy.trust
                        != metadata_string(&self.payload, "review_trust")?
                            .unwrap_or_else(|| "local_candidate".into())
                    || self.execution.omissions
                        != self
                            .payload
                            .get("omissions")
                            .map(|value| serde_json::from_value::<Vec<String>>(value.clone()))
                            .transpose()?
                            .unwrap_or_default()
                    || serde_json::to_value(&self.evidence_refs)?
                        != serde_json::to_value(
                            self.payload
                                .get("artifacts")
                                .map(|value| serde_json::from_value::<Vec<Artifact>>(value.clone()))
                                .transpose()?
                                .unwrap_or_default(),
                        )?
                {
                    bail!("Metadata source digest conflicts with payload");
                }
            }
        }
        Ok(())
    }

    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let envelope: Self = serde_json::from_slice(bytes)?;
        envelope.validate()?;
        Ok(envelope)
    }
}
