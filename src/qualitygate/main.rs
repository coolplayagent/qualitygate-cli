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
    let format = cli.output_format();
    match cli.run().await {
        Ok((output, code)) => {
            println!("{output}");
            std::process::ExitCode::from(code)
        }
        Err(error) => {
            println!(
                "{}",
                qualitygate::interfaces::render_incomplete(&format!("{error:#}"), format)
            );
            std::process::ExitCode::from(2)
        }
    }
}
