//! Evidence-linked lifecycle operations always target a separate candidate.

use crate::{
    config::{
        policy_candidates,
        policy_store::{Store, now},
        rule_management::Mutation,
    },
    domain::{
        evolution::{Actor, ActorKind},
        rule_lifecycle::{RuleLifecycle, RuleState},
    },
};
use anyhow::Result;
use clap::Args;
use std::path::PathBuf;

#[derive(Debug, Args)]
pub(super) struct Change {
    rule_id: String,
    #[arg(long)]
    candidate: String,
    #[arg(long)]
    actor: String,
    #[arg(long, value_enum, default_value = "agent")]
    actor_kind: ActorKind,
    #[arg(long)]
    reason: String,
}

pub(super) async fn run(
    root: PathBuf,
    change: Change,
    state: RuleState,
) -> Result<serde_json::Value> {
    tokio::task::spawn_blocking(move || {
        let (_, candidate) = policy_candidates::candidate(&Store::open(&root)?, &change.candidate)?;
        let actor = Actor {
            id: change.actor,
            kind: change.actor_kind,
        };
        let record = RuleLifecycle {
            state,
            actor: actor.clone(),
            reason: change.reason,
            evidence_refs: candidate.evidence_refs,
            timestamp: now()?,
        };
        policy_candidates::mutate(
            &root,
            &change.candidate,
            actor,
            Mutation::Lifecycle {
                id: change.rule_id,
                record,
            },
        )
    })
    .await?
}
