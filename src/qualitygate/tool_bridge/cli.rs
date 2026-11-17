//! Offline tool-report conversion; no tool invocation or acceptance authority.

use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "qualitygate-tool-bridge",
    version,
    about = "Convert captured tool reports to unbound typed facts; never executes or installs tools"
)]
struct Cli {
    /// Captured upstream EDN format.
    #[arg(long, value_parser = ["crapper", "dryer"])]
    format: String,
    /// Bounded regular file containing the raw upstream report.
    #[arg(long)]
    input: PathBuf,
    /// Bounded JSON context declaring inventory and provenance; declarations remain untrusted.
    #[arg(long)]
    context: PathBuf,
}

pub struct Output {
    pub text: String,
    pub stderr: bool,
    pub code: u8,
}

pub fn entry() -> Output {
    render(Cli::try_parse())
}

fn render(parsed: Result<Cli, clap::Error>) -> Output {
    let cli = match parsed {
        Ok(cli) => cli,
        Err(error) => {
            return Output {
                code: if error.use_stderr() { 2 } else { 0 },
                stderr: error.use_stderr(),
                text: error.to_string(),
            };
        }
    };
    match crate::tool_bridge::conversion::convert(&cli.format, &cli.input, &cli.context) {
        Ok(text) => Output { text, stderr: false, code: 0 },
        Err(error) => Output {
            text: serde_json::json!({"schema_version":1,"status":"incomplete","error":format!("{error:#}")}).to_string(),
            stderr: false,
            code: 2,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_and_version_succeed_without_inputs_and_invalid_commands_are_incomplete() {
        for option in ["--help", "--version"] {
            let output = render(Cli::try_parse_from(["qualitygate-tool-bridge", option]));
            assert_eq!(output.code, 0);
            assert!(!output.stderr);
            assert!(output.text.contains("qualitygate-tool-bridge"));
        }
        for args in [vec!["bridge"], vec!["bridge", "--format", "mutator"]] {
            let output = render(Cli::try_parse_from(args));
            assert_eq!(output.code, 2);
            assert!(output.stderr);
        }
    }

    #[test]
    fn conversion_failure_is_structured_and_does_not_claim_acceptance() {
        let output = render(Cli::try_parse_from([
            "bridge",
            "--format",
            "dryer",
            "--input",
            "missing-bridge-report.edn",
            "--context",
            "missing-bridge-context.json",
        ]));
        assert_eq!(output.code, 2);
        assert!(!output.stderr);
        let value: serde_json::Value = serde_json::from_str(&output.text).unwrap();
        assert_eq!(value["status"], "incomplete");
        assert!(value.get("gate").is_none());
    }
}
