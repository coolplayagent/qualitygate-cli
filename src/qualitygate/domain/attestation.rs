//! Signed manual decisions and their exact verification subject; no I/O.

use super::{PolicyEvidence, SnapshotIdentity};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManualSubject {
    pub repository: String,
    pub snapshot: SnapshotBinding,
    pub policy: PolicyBinding,
    pub check_id: String,
    pub task: Option<TaskBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotBinding {
    pub mode: String,
    pub base: String,
    pub head: String,
    pub content_digest: String,
    pub path_filter: Option<String>,
    pub merge_request: Option<ComparisonBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComparisonBinding {
    pub provider: String,
    pub repository: String,
    pub number: u64,
    pub source_head: String,
    pub target_head: String,
    pub merge_base: String,
    pub source_branch: String,
    pub target_branch: String,
}

impl SnapshotBinding {
    pub fn new(snapshot: &SnapshotIdentity, path_filter: Option<String>) -> Self {
        Self {
            mode: snapshot.mode.clone(),
            base: snapshot.base.clone(),
            head: snapshot.head.clone(),
            content_digest: snapshot.content_digest.clone(),
            path_filter,
            merge_request: snapshot.merge_request.as_ref().map(|mr| ComparisonBinding {
                provider: mr.provider.clone(),
                repository: mr.repository.clone(),
                number: mr.number,
                source_head: mr.source_head.clone(),
                target_head: mr.target_head.clone(),
                merge_base: mr.merge_base.clone(),
                source_branch: mr.source_branch.clone(),
                target_branch: mr.target_branch.clone(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyBinding {
    pub config_digest: String,
    pub rules_digest: String,
    pub task_contract_digest: Option<String>,
}

impl From<&PolicyEvidence> for PolicyBinding {
    fn from(policy: &PolicyEvidence) -> Self {
        Self {
            config_digest: policy.config_digest.clone(),
            rules_digest: policy.rules_digest.clone(),
            task_contract_digest: policy.task_contract_digest.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskBinding {
    pub task_id: String,
    pub acceptance_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManualRecord {
    pub schema_version: u32,
    pub record_id: String,
    pub subject: ManualSubject,
    pub reviewer: String,
    pub decision: ManualDecision,
    pub reason: String,
    pub issued_at: u64,
    pub expires_at: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManualDecision {
    Approved,
    Rejected,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_subject_binds_comparison_and_policy_without_volatile_api_payloads() {
        let mut snapshot: SnapshotIdentity = serde_json::from_value(serde_json::json!({
            "mode":"merge_request","base":"base","head":"head","content_digest":"sha256:code",
            "merge_request":{"provider":"github","url":"https://example.test/pull/1","repository":"owner/repo","number":1,
                "source_branch":"feature","target_branch":"main","source_head":"head","target_head":"target",
                "merge_base":"base","api_response_digests":["sha256:response"]}
        })).unwrap();
        let original = SnapshotBinding::new(&snapshot, None);
        snapshot
            .merge_request
            .as_mut()
            .unwrap()
            .api_response_digests
            .push("new-response".into());
        assert_eq!(SnapshotBinding::new(&snapshot, None), original);
        snapshot.merge_request.as_mut().unwrap().target_head = "updated-target".into();
        assert_ne!(SnapshotBinding::new(&snapshot, None), original);
        assert_ne!(
            SnapshotBinding::new(&snapshot, Some("src".into())),
            SnapshotBinding::new(&snapshot, None)
        );
        let policy = PolicyEvidence {
            resolved_commit: None,
            task_contract_source: None,
            source_reviews: Default::default(),
            source: "trusted".into(),
            config_digest: "config".into(),
            rules_digest: "rules".into(),
            task_contract_digest: Some("task".into()),
            trust: "caller_supplied_ref".into(),
            changes: vec![],
        };
        let binding = PolicyBinding::from(&policy);
        assert_eq!(binding.task_contract_digest.as_deref(), Some("task"));
    }
}
