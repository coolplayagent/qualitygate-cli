//! Pure records for evidence-driven policy changes. None of these records grants trust.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[cfg(test)]
#[path = "evolution_tests.rs"]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum ActorKind {
    Human,
    Agent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Actor {
    pub id: String,
    pub kind: ActorKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    ConversationCorrection,
    CheckReport,
    Review,
    Failure,
    Usage,
    CapabilityGap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Sensitivity {
    Public,
    Internal,
    Restricted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceRecord {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub case: Option<super::case_provenance::CaseProvenance>,
    pub schema_version: u32,
    pub kind: EvidenceKind,
    pub source_digest: String,
    pub scope: String,
    pub timestamp: u64,
    pub actor: Actor,
    pub claims: Vec<String>,
    pub sensitivity: Sensitivity,
    pub rule_ids: Vec<String>,
    pub known_limits: Vec<String>,
    pub unverified_assumptions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyFile {
    pub digest: String,
    pub executable: bool,
}

/// All policy-owned bytes and the frozen rule catalog; source code is captured separately.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyVersion {
    pub schema_version: u32,
    pub config_path: String,
    pub files: BTreeMap<String, PolicyFile>,
    pub catalog_digest: String,
    pub source_commit: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevisionStatus {
    Draft,
    Candidate,
    Validating,
    Approved,
    Active,
    Superseded,
    RolledBack,
    Rejected,
    Abandoned,
    Deprecated,
    Retired,
    Revoked,
}

/// Each edit is a new content-addressed revision. The original policy parent never changes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyRevision {
    pub schema_version: u32,
    pub parent_policy_digest: String,
    pub previous_revision: Option<String>,
    pub policy_digest: String,
    pub patch: Vec<serde_json::Value>,
    pub status: RevisionStatus,
    pub created_by: Actor,
    pub reason: String,
    pub evidence_refs: Vec<String>,
    pub created_at: u64,
    pub evaluation_ref: Option<String>,
    pub approval_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyTransition {
    pub sequence: u64,
    pub previous: Option<String>,
    pub action: String,
    pub subject: String,
    pub actor: Actor,
    pub reason: String,
    pub timestamp: u64,
    pub from_policy: Option<String>,
    pub to_policy: Option<String>,
}

pub fn valid_digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    })
}

pub fn validate_text(value: &str, max: usize) -> Result<(), String> {
    if value.trim().is_empty()
        || value.len() > max
        || value
            .chars()
            .any(|c| c.is_control() && c != '\n' && c != '\t')
    {
        return Err(format!(
            "Text must be nonempty, at most {max} bytes and contain no unsafe control characters"
        ));
    }
    Ok(())
}

impl Actor {
    pub fn validate(&self) -> Result<(), String> {
        validate_text(&self.id, 256)
    }
}

impl EvidenceRecord {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 || !valid_digest(&self.source_digest) || self.timestamp == 0 {
            return Err(
                "Evidence requires version 1, a SHA-256 source digest and timestamp".into(),
            );
        }
        self.actor.validate()?;
        if let Some(case) = &self.case {
            case.validate()?;
        }
        validate_text(&self.scope, 1024)?;
        if self.claims.is_empty()
            || self.claims.len() > 128
            || self.rule_ids.len() > 256
            || self.known_limits.len() > 128
            || self.unverified_assumptions.len() > 128
        {
            return Err("Evidence requires 1..128 claims and bounded rule/limitation lists".into());
        }
        for value in self
            .claims
            .iter()
            .chain(&self.known_limits)
            .chain(&self.unverified_assumptions)
        {
            validate_text(value, 4096)?;
        }
        let mut ids = std::collections::BTreeSet::new();
        for id in &self.rule_ids {
            validate_text(id, 256)?;
            if !ids.insert(id) {
                return Err("Duplicate evidence rule ID".into());
            }
        }
        if self.kind == EvidenceKind::CapabilityGap && !self.rule_ids.is_empty() {
            return Err(
                "Capability gaps must describe missing capability without invented rule IDs".into(),
            );
        }
        Ok(())
    }
}

impl PolicyRevision {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1
            || self.created_at == 0
            || !valid_digest(&self.parent_policy_digest)
            || !valid_digest(&self.policy_digest)
        {
            return Err(
                "Candidate requires version 1, immutable policy digests and timestamp".into(),
            );
        }
        self.created_by.validate()?;
        validate_text(&self.reason, 4096)?;
        if self.evidence_refs.is_empty() || self.evidence_refs.len() > 128 || self.patch.len() > 256
        {
            return Err(
                "Candidate requires 1..128 evidence references and at most 256 edits".into(),
            );
        }
        let mut refs = std::collections::BTreeSet::new();
        for reference in &self.evidence_refs {
            if !valid_digest(reference) || !refs.insert(reference) {
                return Err("Invalid or duplicate candidate evidence reference".into());
            }
        }
        for reference in [
            &self.previous_revision,
            &self.evaluation_ref,
            &self.approval_ref,
        ]
        .into_iter()
        .flatten()
        {
            if !valid_digest(reference) {
                return Err("Invalid candidate record reference".into());
            }
        }
        Ok(())
    }

    pub fn editable(&self) -> bool {
        matches!(
            self.status,
            RevisionStatus::Draft | RevisionStatus::Candidate
        )
    }
}
