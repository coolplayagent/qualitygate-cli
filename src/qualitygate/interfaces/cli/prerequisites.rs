use super::{Categories, Command, Rules};
use crate::interfaces::policy::{Candidate, Evidence, Policy};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Requirements {
    Independent,
    Discovery,
    OptionalPolicy,
    EffectivePolicy,
    LocalPolicy,
    Snapshot,
    Files,
    Archive,
    PolicyReference,
}

pub(super) fn requirements(command: &Command) -> Requirements {
    match command {
        Command::Schema { .. } | Command::Selfcheck { .. } | Command::SelfcheckProbe { .. } => {
            Requirements::Independent
        }
        Command::Init { .. } => Requirements::Discovery,
        Command::Config { .. } => Requirements::EffectivePolicy,
        Command::Check(_) => Requirements::Snapshot,
        Command::Pilot { .. } | Command::Judgment { .. } => Requirements::Files,
        Command::Rules { command } => match command {
            Rules::Schema => Requirements::Independent,
            Rules::List { .. }
            | Rules::Describe { .. }
            | Rules::Context {
                policy_ref: None, ..
            }
            | Rules::Categories {
                command: None | Some(Categories::List),
            } => Requirements::OptionalPolicy,
            Rules::Context {
                policy_ref: Some(_),
                ..
            } => Requirements::PolicyReference,
            Rules::Enable { .. }
            | Rules::Disable { .. }
            | Rules::Configure { .. }
            | Rules::Assign { .. }
            | Rules::Unassign { .. }
            | Rules::Categories {
                command:
                    Some(
                        Categories::Create { .. }
                        | Categories::Rename { .. }
                        | Categories::Delete { .. },
                    ),
            } => Requirements::LocalPolicy,
            Rules::Source { .. } | Rules::Validate { .. } | Rules::Generate { .. } => {
                Requirements::Files
            }
            Rules::History { .. }
            | Rules::Revalidate(_)
            | Rules::Demote(_)
            | Rules::Deprecate(_)
            | Rules::Retire(_)
            | Rules::Revoke(_) => Requirements::Archive,
        },
        Command::Policy { command } => match command {
            Policy::Evaluator => Requirements::Independent,
            Policy::Evidence {
                command: Evidence::Add { .. },
            } => Requirements::Files,
            Policy::Candidate {
                command: Candidate::Create { .. },
            } => Requirements::PolicyReference,
            Policy::Candidate {
                command:
                    Candidate::ApprovalSubject { .. }
                    | Candidate::Approve { .. }
                    | Candidate::Promote { .. }
                    | Candidate::Validate { .. }
                    | Candidate::Abandon { .. }
                    | Candidate::Show { .. }
                    | Candidate::List(_)
                    | Candidate::Rules { .. }
                    | Candidate::Reject { .. },
            }
            | Policy::Evidence {
                command: Evidence::Show { .. } | Evidence::List(_),
            }
            | Policy::RollbackSubject { .. }
            | Policy::Rollback { .. }
            | Policy::Effectiveness { .. }
            | Policy::Record { .. }
            | Policy::Evaluation { .. }
            | Policy::Show { .. }
            | Policy::History { .. } => Requirements::Archive,
        },
    }
}
