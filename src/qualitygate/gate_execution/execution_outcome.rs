//! Typed execution evidence; report metadata is only a display projection.

use crate::domain::{CheckResult, ExecutionStatus, ProjectFacts, Verdict};
use crate::tool_evidence::contracts::ToolFacts;

#[derive(Default)]
pub(crate) struct ProducedEvidence {
    pub projects: Vec<ProjectFacts>,
    pub tool_facts: Option<ToolFacts>,
    pub coverage: Option<crate::tool_evidence::contracts::CoverageEvidence>,
    pub receipt: Option<super::fact_execution::Receipt>,
}

pub(crate) struct ExecutionOutcome {
    pub check_result: CheckResult,
    pub evidence: ProducedEvidence,
}

impl ExecutionOutcome {
    pub(crate) fn new(check_result: CheckResult, evidence: ProducedEvidence) -> Self {
        let usable = check_result.execution.status == ExecutionStatus::Completed
            && check_result.verdict == Some(Verdict::Pass);
        Self {
            check_result,
            evidence: if usable {
                evidence
            } else {
                ProducedEvidence::default()
            },
        }
    }
}
