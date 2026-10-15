//! CLI arguments shared by the primary command and the compatibility example.
use crate::application::agent_host::{FeedbackMode, Options};
use clap::Args;
use std::path::PathBuf;

#[derive(Debug, Args)]
pub struct AgentArgs {
    #[arg(long)]
    qualitygate: PathBuf,
    #[arg(long)]
    base: String,
    #[arg(long)]
    policy_ref: String,
    #[arg(long)]
    task: String,
    /// UTF-8 task instructions, at most 16 KiB.
    #[arg(long)]
    prompt_file: PathBuf,
    /// JSON argv array for an agent accepting the complete prompt on stdin.
    #[arg(long)]
    agent_command: PathBuf,
    #[arg(long)]
    replacement_file: Option<PathBuf>,
    /// Existing evidence directory outside the checked repository.
    #[arg(long)]
    output_dir: PathBuf,
    #[arg(long, default_value = "compact", value_parser = ["compact", "full"])]
    feedback_mode: String,
    #[arg(long, default_value_t=3, value_parser=clap::value_parser!(u8).range(1..=3))]
    max_attempts: u8,
    #[arg(long, default_value_t=1800, value_parser=clap::value_parser!(u64).range(1..=1800))]
    timeout_seconds: u64,
    #[arg(long, default_value = "implementer")]
    role: String,
    /// Previous controller index used only as bounded context; never reuse its completion evidence.
    #[arg(long)]
    resume: Option<PathBuf>,
    #[arg(long, requires = "evidence_dir")]
    trust_store: Option<PathBuf>,
    #[arg(long, requires = "trust_store")]
    evidence_dir: Option<PathBuf>,
}

impl AgentArgs {
    pub fn options(self, root: PathBuf, config: String) -> Options {
        Options {
            root,
            config,
            qualitygate: self.qualitygate,
            base: self.base,
            policy_ref: self.policy_ref,
            task: self.task,
            prompt_file: self.prompt_file,
            agent_command: self.agent_command,
            replacement_file: self.replacement_file,
            output_dir: self.output_dir,
            feedback_mode: if self.feedback_mode == "compact" {
                FeedbackMode::Compact
            } else {
                FeedbackMode::Full
            },
            max_attempts: self.max_attempts,
            timeout_seconds: self.timeout_seconds,
            role: self.role,
            resume: self.resume,
            trust_store: self.trust_store,
            evidence_dir: self.evidence_dir,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    #[derive(Debug, Parser)]
    struct Command {
        #[command(flatten)]
        args: AgentArgs,
    }
    #[test]
    fn cli_rejects_out_of_range_attempt_budgets_with_all_other_inputs_present() {
        let error = Command::try_parse_from([
            "agent-run",
            "--qualitygate",
            "cli",
            "--base",
            "HEAD",
            "--policy-ref",
            "HEAD",
            "--task",
            "task.yaml",
            "--prompt-file",
            "prompt.txt",
            "--agent-command",
            "agent.json",
            "--output-dir",
            "/tmp",
            "--max-attempts",
            "4",
        ])
        .unwrap_err();
        assert_eq!(error.kind(), clap::error::ErrorKind::ValueValidation);
    }
}
