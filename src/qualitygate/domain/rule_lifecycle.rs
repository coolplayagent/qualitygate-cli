//! Published rule lifecycle metadata remains replayable in older immutable packages.

use super::evolution::{Actor, valid_digest, validate_text};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum RuleState {
    Revalidate,
    Demoted,
    Deprecated,
    Retired,
    Revoked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleLifecycle {
    pub state: RuleState,
    pub actor: Actor,
    pub reason: String,
    pub evidence_refs: Vec<String>,
    pub timestamp: u64,
}

impl RuleLifecycle {
    pub fn validate(&self) -> Result<(), String> {
        self.actor.validate()?;
        validate_text(&self.reason, 4096)?;
        if self.timestamp == 0
            || self.evidence_refs.is_empty()
            || self.evidence_refs.len() > 64
            || self
                .evidence_refs
                .iter()
                .any(|reference| !valid_digest(reference))
        {
            return Err(
                "Rule lifecycle requires bounded motivating evidence and a timestamp".into(),
            );
        }
        Ok(())
    }
}
