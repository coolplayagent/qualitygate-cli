use super::{Decision, Gate, VerificationBoundary};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureCode {
    InvalidArguments,
    RootUnavailable,
    RepositoryNotInitialized,
    PolicySnapshotMissing,
    InputMissing,
    InputUnreadable,
    ConfigInvalid,
    RuleAssetsUnavailable,
    GitUnavailable,
    GitRepositoryInvalid,
    GitRefUnavailable,
    SnapshotUnavailable,
    PlanInvalid,
    ToolUnavailable,
    ToolProbeFailed,
    ToolIdentityChanged,
    WorkspaceUnavailable,
    StorageUnavailable,
    LockUnavailable,
    ArchiveUnavailable,
    EvidenceInvalid,
    PolicyAuthenticationFailed,
    RemoteUnavailable,
    UnknownError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Entry,
    Policy,
    Inputs,
    Prepare,
    Execution,
    Evidence,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RecoveryAction {
    Command { message: String, argv: Vec<String> },
    Instruction { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrerequisiteIssue {
    pub code: FailureCode,
    pub phase: Phase,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub check_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cause: Option<String>,
    pub next_actions: Vec<RecoveryAction>,
}

impl PrerequisiteIssue {
    pub fn new(code: FailureCode, phase: Phase, message: impl Into<String>) -> Self {
        Self {
            code,
            phase,
            message: message.into(),
            resource: None,
            check_id: None,
            cause: None,
            next_actions: Vec::new(),
        }
    }

    pub fn resource(mut self, resource: impl Into<String>) -> Self {
        self.resource = Some(resource.into());
        self
    }

    pub fn instruction(mut self, message: impl Into<String>) -> Self {
        self.next_actions.push(RecoveryAction::Instruction {
            message: message.into(),
        });
        self
    }

    pub fn wrap(mut self, error: anyhow::Error) -> anyhow::Error {
        if error.downcast_ref::<Self>().is_some() {
            return error;
        }
        self.cause = Some(format!("{error:#}"));
        error.context(self)
    }

    pub fn from_error(error: &anyhow::Error) -> Self {
        let mut issue = error.downcast_ref::<Self>().cloned().unwrap_or_else(|| {
            Self::new(
                FailureCode::UnknownError,
                Phase::Execution,
                error.to_string(),
            )
            .instruction("Inspect the reported cause and retry the same command after repair.")
        });
        issue.cause = Some(format!("{error:#}"));
        issue
    }
}

impl std::fmt::Display for PrerequisiteIssue {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)?;
        if let Some(cause) = &self.cause {
            write!(formatter, ": {cause}")?;
        }
        Ok(())
    }
}

impl std::error::Error for PrerequisiteIssue {}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandError {
    pub schema_version: u32,
    pub kind: String,
    pub gate: Gate,
    pub verification: VerificationBoundary,
    pub issues: Vec<PrerequisiteIssue>,
}

impl CommandError {
    pub fn new(issue: PrerequisiteIssue) -> Self {
        let gate = Gate {
            complete: false,
            decision: Decision::Incomplete,
            blockers: vec![issue.message.clone()],
        };
        let verification = VerificationBoundary::for_check(&gate, &[], "full", false);
        Self {
            schema_version: 1,
            kind: "command_error".into(),
            gate,
            verification,
            issues: vec![issue],
        }
    }
}

#[cfg(test)]
#[path = "prerequisites_tests.rs"]
mod tests;
