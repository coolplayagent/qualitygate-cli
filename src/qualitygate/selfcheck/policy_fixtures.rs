//! Reviewable policy regression inputs; none of these fixtures grants production trust.

use crate::domain::{
    CheckResult,
    policy_evaluation::{CaseKind, Expected},
    rule_lifecycle::RuleState,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum EvolutionFixture {
    Context {
        content: String,
        category: Option<String>,
    },
    Acceptance {
        content: String,
    },
    Paired {
        pairs: Vec<Pair>,
        expected_count: usize,
        min_improvements: usize,
        #[serde(default)]
        invalid: Vec<String>,
    },
    Authorization {
        rollback: bool,
        change: AuthorizationChange,
    },
    Candidate {
        action: CandidateAction,
    },
    Workflow {
        scenario: WorkflowScenario,
        jobs: u16,
    },
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Pair {
    pub kind: CaseKind,
    pub expectations: BTreeMap<String, Expected>,
    pub baseline: ReportInput,
    pub candidate: ReportInput,
    pub snapshot_present: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReportInput {
    pub checks: Vec<CheckResult>,
    pub required: Vec<String>,
    #[serde(default)]
    pub pending: Vec<String>,
    #[serde(default)]
    pub invalid: Vec<String>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorizationChange {
    None,
    Rejected,
    TamperedSignature,
    WrongPayloadType,
    UnknownKey,
    SelfApproval,
    AgentApprover,
    WrongIdentity,
    WrongSubject,
    Revoked,
    Expired,
    Future,
    ExcessiveLifetime,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CandidateAction {
    Edit,
    Reject,
    MissingEvidence,
    SourceMismatch,
    CorruptObject,
    WrongRecordKind,
    Lifecycle { state: RuleState },
    LifecycleWrongActor,
    LifecycleWrongEvidence,
    UnapprovedPromotion,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowScenario {
    Promote,
    Block,
    MissingTool,
    Timeout,
    RuleBudget,
    WrongBaseline,
    Rollback,
    StaleRollback,
}
