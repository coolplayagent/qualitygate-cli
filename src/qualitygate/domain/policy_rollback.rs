//! Independent authorization of an auditable return to a previously selected policy.
use super::{ManualDecision, evolution::Actor};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RollbackSubject {
    pub repository: String,
    pub from_policy: String,
    pub from_authorization: String,
    pub from_since: u64,
    pub to_policy: String,
    pub history_cursor: String,
    pub target_history_ref: String,
    pub trust_digest: String,
    pub evaluator_digest: String,
    pub proposed_by: Actor,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RollbackApproval {
    pub schema_version: u32,
    pub subject: RollbackSubject,
    pub approver: Actor,
    pub decision: ManualDecision,
    pub issued_at: u64,
    pub expires_at: u64,
    pub reason: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovedRollback {
    pub approval: RollbackApproval,
    pub signer_key_id: String,
    pub public_key_digest: String,
    pub envelope_ref: String,
    pub trust_source: String,
}
