use clap::Parser;
use qualitygate::interfaces::cli::Cli;

#[tokio::main]
async fn main() -> std::process::ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            let code = if error.use_stderr() { 2 } else { 0 };
            let _ = error.print();
            return std::process::ExitCode::from(code);
        }
    };
    match cli.run().await {
        Ok((output, code)) => {
            println!("{output}");
            std::process::ExitCode::from(code)
        }
        Err(error) => {
            println!(
                "{}",
                serde_json::json!({"schema_version":1,"gate":{"complete":false,"decision":"incomplete","blockers":[format!("{error:#}")]}})
            );
            std::process::ExitCode::from(2)
        }
    }
}
