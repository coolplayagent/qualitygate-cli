//! Signed authorization binds the exact validated candidate and its parent.

use super::{ManualDecision, evolution::Actor};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyApprovalSubject {
    pub repository: String,
    pub candidate_id: String,
    pub candidate_revision: String,
    pub candidate_policy: String,
    pub parent_policy: String,
    pub evaluation_ref: String,
    pub suite_digest: String,
    pub trust_digest: String,
    pub evaluator_digest: String,
    pub generation_actor: Actor,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyApproval {
    pub schema_version: u32,
    pub subject: PolicyApprovalSubject,
    pub approver: Actor,
    pub decision: ManualDecision,
    pub issued_at: u64,
    pub expires_at: u64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovedPolicy {
    pub approval: PolicyApproval,
    pub signer_key_id: String,
    pub public_key_digest: String,
    pub envelope_ref: String,
    pub trust_source: String,
}
