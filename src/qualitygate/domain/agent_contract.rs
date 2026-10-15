//! Source-bound instructions for the consuming agent, separate from code verdicts.
use super::{PolicyEvidence, SnapshotIdentity, SourceReview, constraint::Source};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Obligation {
    pub schema_version: u32,
    pub id: String,
    pub version: u32,
    pub source: Source,
    pub required: bool,
    pub scope: Scope,
    pub triggers: Vec<Event>,
    pub must: Vec<String>,
    pub must_not: Vec<String>,
    pub enforcement: Enforcement,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adoption: Option<SourceReview>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scope {
    /// Empty lists apply to every role/task in the selected repository policy.
    pub roles: Vec<String>,
    pub tasks: Vec<String>,
}

impl Scope {
    pub fn applies(&self, role: &str, task: Option<&str>) -> bool {
        (self.roles.is_empty() || self.roles.iter().any(|v| v == role))
            && (self.tasks.is_empty() || task.is_some_and(|id| self.tasks.iter().any(|v| v == id)))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Event {
    Start,
    Repair,
    Resume,
    Handoff,
    BeforeTool,
    AfterChange,
    BeforeDelivery,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Enforcement {
    /// Delivery of instructions is observable; compliance is not established.
    Instruction,
    /// Verify only the facts supported by the named checks, not agent intent.
    Checks { checks: Vec<String> },
    /// A host must implement this exact control, or retain a capability gap.
    Host { control: Control },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Control {
    PromptInjection,
    PolicyIntegrity,
    FinalFullCheck,
    ImmutableDelivery,
    ToolAuthorization,
    FinalResponse,
    InProcessContextRecovery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Ready,
    Unadopted,
    StaleAdoption,
    SourceInvalid,
    UnavailableCheck,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundObligation {
    pub obligation: Obligation,
    pub binding_digest: String,
    pub status: Status,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Projection {
    pub schema_version: u32,
    pub kind: String,
    pub repository: String,
    pub role: String,
    pub task_id: Option<String>,
    pub snapshot: SnapshotIdentity,
    pub policy: PolicyEvidence,
    /// Binds policy, task, role and definitions; code changes have their own snapshot binding.
    pub contract_digest: String,
    pub prompt: String,
    pub prompt_digest: String,
    /// Ready for injection, never a statement of compliance or host enforcement.
    pub ready: bool,
    pub obligations: Vec<BoundObligation>,
    pub gaps: Vec<String>,
    pub known_limits: Vec<String>,
}

/// Detect exact textual contradictions. Semantic conflicts still need review.
pub fn conflicts(obligations: &[BoundObligation]) -> Vec<String> {
    let mut must = std::collections::BTreeMap::new();
    for bound in obligations.iter().filter(|b| b.status == Status::Ready) {
        for text in &bound.obligation.must {
            must.insert(text.trim(), &bound.obligation.id);
        }
    }
    obligations
        .iter()
        .filter(|b| b.status == Status::Ready)
        .flat_map(|b| {
            b.obligation.must_not.iter().filter_map(|text| {
                must.get(text.trim()).map(|other| {
                    format!(
                        "Conflicting must/must_not instructions: {other} and {}",
                        b.obligation.id
                    )
                })
            })
        })
        .take(128)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scope_requires_the_selected_role_and_task_without_guessing_missing_context() {
        let scope = Scope {
            roles: vec!["implementer".into()],
            tasks: vec!["task-a".into()],
        };
        assert!(scope.applies("implementer", Some("task-a")));
        assert!(!scope.applies("reviewer", Some("task-a")));
        assert!(!scope.applies("implementer", None));
        assert!(!scope.applies("implementer", Some("task-b")));
        assert!(Scope::default().applies("reviewer", None));
    }
}
