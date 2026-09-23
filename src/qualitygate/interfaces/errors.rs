use super::{cli::Format, render::escape_controls};
use crate::domain::prerequisites::{CommandError, FailureCode, PrerequisiteIssue, RecoveryAction};

pub(super) fn invalid_arguments(message: &str) -> anyhow::Error {
    PrerequisiteIssue::new(
        FailureCode::InvalidArguments,
        crate::domain::prerequisites::Phase::Entry,
        message,
    )
    .instruction("Correct the command arguments; consult the selected command's --help.")
    .into()
}

#[derive(Debug, Clone)]
pub struct ErrorContext {
    pub root: String,
    pub config: String,
    pub format: Format,
    pub max_bytes: usize,
}

impl ErrorContext {
    pub fn render(&self, error: &anyhow::Error) -> String {
        let mut issue = PrerequisiteIssue::from_error(error);
        if issue.code == FailureCode::RepositoryNotInitialized {
            issue.next_actions = vec![RecoveryAction::Command {
                message: "Run: qualitygate init with the selected root and config; review the candidate before checking.".into(),
                argv: vec![
                    "qualitygate".into(), "--root".into(), self.root.clone(),
                    "--config".into(), self.config.clone(), "init".into(),
                    "--format".into(), "json".into(),
                ],
            }];
        }
        let mut report = CommandError::new(issue);
        bound(&mut report, self.max_bytes);
        if self.format == Format::Json {
            return serde_json::to_string(&report).expect("serializable command error");
        }
        let issue = &report.issues[0];
        let mut output = format!(
            "{}\nGate code: 2 | complete: false\nBlocker: {}\nCode: {}\n",
            report.verification.conclusion,
            escape_controls(&issue.message),
            serde_json::to_value(issue.code)
                .expect("serializable code")
                .as_str()
                .unwrap(),
        );
        for action in &issue.next_actions {
            match action {
                RecoveryAction::Command { message, argv } => {
                    output.push_str(&format!(
                        "{}\nargv: {}\n",
                        escape_controls(message),
                        serde_json::to_string(argv).expect("serializable argv")
                    ));
                }
                RecoveryAction::Instruction { message } => {
                    output.push_str(&format!("Next: {}\n", escape_controls(message)));
                }
            }
        }
        if let Some(cause) = &issue.cause {
            output.push_str(&format!("Cause: {}\n", escape_controls(cause)));
        }
        output.push_str(&format!(
            "known_limits: {}\n",
            report.verification.known_limits.join(" ")
        ));
        output
    }
}

fn shorten(value: &mut String, limit: usize) {
    if value.len() > limit {
        let mut boundary = limit;
        while !value.is_char_boundary(boundary) {
            boundary -= 1;
        }
        value.truncate(boundary);
        value.push_str(" [truncated]");
    }
}

fn bound(report: &mut CommandError, limit: usize) {
    let fits = |report: &CommandError| {
        serde_json::to_vec(report)
            .expect("serializable command error")
            .len()
            < limit
    };
    if fits(report) {
        return;
    }
    let issue = &mut report.issues[0];
    issue.cause = None;
    issue.resource = None;
    shorten(&mut issue.message, 512);
    report.gate.blockers = vec![issue.message.clone()];
    if fits(report) {
        return;
    }
    report.issues[0].next_actions = vec![RecoveryAction::Instruction {
        message: "Repair the reported prerequisite. Retry using the original root/config; command arguments were omitted to respect the output budget.".into(),
    }];
    report.issues[0].check_id = None;
    shorten(&mut report.issues[0].message, 128);
    report.gate.blockers = vec![report.issues[0].message.clone()];
}
