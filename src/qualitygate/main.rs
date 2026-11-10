use clap::Parser;
use qualitygate::interfaces::cli::Cli;

#[tokio::main]
async fn main() -> std::process::ExitCode {
    run(Cli::try_parse()).await
}

async fn run(parsed: Result<Cli, clap::Error>) -> std::process::ExitCode {
    let cli = match parsed {
        Ok(cli) => cli,
        Err(error) => {
            let code = if error.use_stderr() { 2 } else { 0 };
            let _ = error.print();
            return std::process::ExitCode::from(code);
        }
    };
    let format = cli.output_format();
    let failure_context = cli.failure_context();
    match cli.run().await {
        Ok((output, code)) => {
            println!("{output}");
            std::process::ExitCode::from(code)
        }
        Err(error) => {
            println!(
                "{}",
                qualitygate::interfaces::render_failure(&error, format, failure_context).await
            );
            std::process::ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn process_boundary_distinguishes_help_parser_failure_success_and_execution_gap() {
        for (args, expected) in [
            (vec!["qualitygate", "--help"], 0),
            (vec!["qualitygate", "unknown-command"], 2),
            (
                vec!["qualitygate", "--format", "json", "schema", "decision"],
                0,
            ),
        ] {
            assert_eq!(
                run(Cli::try_parse_from(args)).await,
                std::process::ExitCode::from(expected)
            );
        }
        let root = tempfile::tempdir().unwrap();
        let missing = root.path().join("missing-repository");
        assert_eq!(
            run(Cli::try_parse_from([
                "qualitygate",
                "--root",
                missing.to_str().unwrap(),
                "--format",
                "json",
                "config",
                "--show"
            ]))
            .await,
            std::process::ExitCode::from(2)
        );
    }
}
