//! Policy archive command arguments; storage and mutation remain configuration-owned.

use crate::{
    application,
    config::{policy_candidates, rule_management::Mutation},
    domain::{
        Severity,
        evolution::{Actor, ActorKind},
    },
};
use anyhow::Result;
use clap::{Args, Subcommand};
use serde_json::Value;
use std::path::PathBuf;

#[derive(Debug, Subcommand)]
pub(super) enum Policy {
    RollbackSubject {
        #[arg(long)]
        to: String,
        #[arg(long)]
        trust_store: PathBuf,
        #[command(flatten)]
        attribution: Attribution,
    },
    Rollback {
        #[arg(long)]
        to: String,
        #[arg(long)]
        trust_store: PathBuf,
        #[arg(long)]
        approval: PathBuf,
    },
    /// Compare retained evaluations only within matched resource and evaluator groups.
    Effectiveness {
        #[arg(long)]
        policy_ref: Option<String>,
        #[command(flatten)]
        page: Page,
    },
    Record {
        reference: String,
        #[arg(long, value_parser = ["check_report", "validation_attempt", "policy_approval", "candidate", "blob"])]
        kind: String,
    },
    /// Identify this evaluator before an operator authorizes an acceptance epoch.
    Evaluator,
    Evaluation {
        reference: String,
    },
    /// Retain source evidence without treating claims as verified policy.
    Evidence {
        #[command(subcommand)]
        command: Evidence,
    },
    /// Propose isolated, immutable policy revisions.
    Candidate {
        #[command(subcommand)]
        command: Candidate,
    },
    /// Read an immutable policy version by digest.
    Show {
        digest: String,
    },
    /// Read retained policy transitions, newest first, with a bounded cursor.
    History {
        #[arg(long)]
        cursor: Option<String>,
        #[arg(long, default_value_t = 32, value_parser = clap::value_parser!(u16).range(1..=256))]
        limit: u16,
    },
}

#[derive(Debug, Args)]
pub(super) struct Attribution {
    #[arg(long)]
    actor: String,
    #[arg(long, value_enum, default_value = "agent")]
    actor_kind: ActorKind,
}
impl Attribution {
    fn into_actor(self) -> Actor {
        Actor {
            id: self.actor,
            kind: self.actor_kind,
        }
    }
}

#[derive(Debug, Args)]
pub(super) struct Page {
    #[arg(long, default_value_t = 0)]
    offset: usize,
    #[arg(long, default_value_t = 32, value_parser = clap::value_parser!(u16).range(1..=256))]
    limit: u16,
}

#[derive(Debug, Subcommand)]
pub(super) enum Evidence {
    Add {
        #[arg(long)]
        input: String,
        #[arg(long)]
        source: String,
    },
    Show {
        reference: String,
    },
    List(Page),
}

#[derive(Debug, Subcommand)]
pub(super) enum Candidate {
    ApprovalSubject {
        id: String,
        #[arg(long)]
        trust_store: PathBuf,
    },
    Approve {
        id: String,
        #[arg(long)]
        approval: PathBuf,
        #[arg(long)]
        trust_store: PathBuf,
    },
    Promote {
        id: String,
    },
    Validate {
        id: String,
        #[arg(long)]
        baseline: String,
        #[arg(long)]
        task: PathBuf,
        #[arg(long)]
        trust_store: PathBuf,
        #[arg(long)]
        evidence_dir: PathBuf,
        #[arg(long, value_parser = clap::value_parser!(u16).range(1..=8))]
        jobs: Option<u16>,
        #[command(flatten)]
        attribution: Attribution,
    },
    Abandon {
        id: String,
        #[arg(long)]
        reason: String,
        #[command(flatten)]
        attribution: Attribution,
    },
    Create {
        #[arg(long)]
        from_policy_ref: String,
        #[arg(long, required = true)]
        evidence: Vec<String>,
        #[arg(long)]
        reason: String,
        #[command(flatten)]
        attribution: Attribution,
    },
    Show {
        id: String,
    },
    List(Page),
    Rules {
        #[command(subcommand)]
        command: CandidateRules,
    },
    Reject {
        id: String,
        #[arg(long)]
        reason: String,
        #[command(flatten)]
        attribution: Attribution,
    },
}

#[derive(Debug, Subcommand)]
pub(super) enum CandidateRules {
    Enable {
        candidate_id: String,
        rule_id: String,
        #[command(flatten)]
        attribution: Attribution,
    },
    Disable {
        candidate_id: String,
        rule_id: String,
        #[command(flatten)]
        attribution: Attribution,
    },
    Configure {
        candidate_id: String,
        rule_id: String,
        #[arg(long = "param")]
        parameters: Vec<String>,
        #[arg(long, value_enum)]
        severity: Option<Severity>,
        #[arg(long, action = clap::ArgAction::Set)]
        required: Option<bool>,
        #[command(flatten)]
        attribution: Attribution,
    },
}

pub(super) async fn run(
    root: PathBuf,
    config_path: String,
    command: Policy,
) -> Result<(Value, u8)> {
    match command {
        Policy::RollbackSubject {
            to,
            trust_store,
            attribution,
        } => {
            return Ok((
                application::policy_rollback::subject(
                    root,
                    to,
                    trust_store,
                    attribution.into_actor(),
                )
                .await?,
                0,
            ));
        }
        Policy::Rollback {
            to,
            trust_store,
            approval,
        } => {
            return Ok((
                application::policy_rollback::rollback(root, to, trust_store, approval).await?,
                0,
            ));
        }
        Policy::Candidate {
            command: Candidate::ApprovalSubject { id, trust_store },
        } => {
            return Ok((
                application::policy_promotion::subject(root, id, trust_store).await?,
                0,
            ));
        }
        Policy::Candidate {
            command:
                Candidate::Approve {
                    id,
                    approval,
                    trust_store,
                },
        } => return application::policy_promotion::approve(root, id, approval, trust_store).await,
        Policy::Candidate {
            command: Candidate::Promote { id },
        } => return Ok((application::policy_promotion::promote(root, id).await?, 0)),
        _ => {}
    }
    if let Policy::Evaluator = command {
        return Ok((application::policy_validation::evaluator().await?, 0));
    }
    if let Policy::Candidate {
        command:
            Candidate::Validate {
                id,
                baseline,
                task,
                trust_store,
                evidence_dir,
                jobs,
                attribution,
            },
    } = command
    {
        return application::policy_validation::validate(
            application::policy_validation::ValidateOptions {
                root,
                candidate_id: id,
                baseline,
                task,
                trust_store,
                evidence_dir,
                jobs,
                actor: attribution.into_actor(),
            },
        )
        .await;
    }
    if let Policy::Candidate {
        command:
            Candidate::Create {
                from_policy_ref,
                evidence,
                reason,
                attribution,
            },
    } = command
    {
        return Ok((
            application::policy_candidates::create(
                root,
                config_path,
                from_policy_ref,
                policy_candidates::Proposal {
                    actor: attribution.into_actor(),
                    reason,
                    evidence,
                },
            )
            .await?,
            0,
        ));
    }
    tokio::task::spawn_blocking(move || {
        let result = match command {
            Policy::RollbackSubject { .. } | Policy::Rollback { .. } => {
                unreachable!("rollback authorization runs asynchronously")
            }
            Policy::Effectiveness { policy_ref, page } => {
                crate::config::policy_effectiveness::report(
                    &root,
                    policy_ref.as_deref(),
                    page.offset,
                    page.limit.into(),
                )?
            }
            Policy::Record { reference, kind } => {
                crate::config::policy_effectiveness::record(&root, &reference, &kind)?
            }
            Policy::Candidate {
                command:
                    Candidate::ApprovalSubject { .. }
                    | Candidate::Approve { .. }
                    | Candidate::Promote { .. },
            } => unreachable!("trusted approval is orchestrated asynchronously"),
            Policy::Evaluator
            | Policy::Candidate {
                command: Candidate::Validate { .. },
            } => unreachable!("evaluation runs asynchronously"),
            Policy::Evaluation { reference } => {
                crate::config::policy_validation::evaluation(&root, &reference)?
            }
            Policy::Candidate {
                command:
                    Candidate::Abandon {
                        id,
                        reason,
                        attribution,
                    },
            } => crate::config::policy_validation::abandon(
                &root,
                &id,
                attribution.into_actor(),
                &reason,
            )?,
            Policy::Candidate {
                command: Candidate::Create { .. },
            } => unreachable!("parent snapshot acquisition is asynchronous"),
            Policy::Evidence {
                command: Evidence::Add { input, source },
            } => policy_candidates::retain_from_files(&root, &input, &source)?,
            Policy::Evidence {
                command: Evidence::Show { reference },
            } => policy_candidates::show(&root, "evidence", &reference)?,
            Policy::Evidence {
                command: Evidence::List(page),
            } => policy_candidates::list(&root, "evidence", page.offset, page.limit.into())?,
            Policy::Candidate {
                command: Candidate::Show { id },
            } => policy_candidates::show(&root, "candidate", &id)?,
            Policy::Candidate {
                command: Candidate::List(page),
            } => policy_candidates::list(&root, "candidate", page.offset, page.limit.into())?,
            Policy::Candidate {
                command:
                    Candidate::Reject {
                        id,
                        reason,
                        attribution,
                    },
            } => policy_candidates::reject(&root, &id, attribution.into_actor(), &reason)?,
            Policy::Candidate {
                command: Candidate::Rules { command },
            } => {
                let (id, actor, mutation) = match command {
                    CandidateRules::Enable {
                        candidate_id,
                        rule_id,
                        attribution,
                    } => (
                        candidate_id,
                        attribution.into_actor(),
                        Mutation::Enable(rule_id),
                    ),
                    CandidateRules::Disable {
                        candidate_id,
                        rule_id,
                        attribution,
                    } => (
                        candidate_id,
                        attribution.into_actor(),
                        Mutation::Disable(rule_id),
                    ),
                    CandidateRules::Configure {
                        candidate_id,
                        rule_id,
                        parameters,
                        severity,
                        required,
                        attribution,
                    } => (
                        candidate_id,
                        attribution.into_actor(),
                        Mutation::Configure {
                            id: rule_id,
                            parameters,
                            severity,
                            required,
                        },
                    ),
                };
                policy_candidates::mutate(&root, &id, actor, mutation)?
            }
            Policy::Show { digest } => policy_candidates::show(&root, "policy", &digest)?,
            Policy::History { cursor, limit } => {
                policy_candidates::history(&root, cursor.as_deref(), limit.into())?
            }
        };
        Ok((result, 0))
    })
    .await?
}
