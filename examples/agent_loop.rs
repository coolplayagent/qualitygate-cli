//! Compatibility entry point for the reusable Rust consuming-agent controller.
use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Parser)]
pub(crate) struct Options {
    #[arg(long)]
    root: PathBuf,
    #[command(flatten)]
    args: qualitygate::interfaces::agent_run::AgentArgs,
}

#[cfg_attr(
    test,
    allow(
        dead_code,
        reason = "this compatibility entry point is exercised by the integration target"
    )
)]
pub(crate) async fn run(options: Options) -> anyhow::Result<serde_json::Value> {
    qualitygate::application::agent_host::run(
        options
            .args
            .options(options.root, "qualitygate.yaml".into()),
    )
    .await
}

#[cfg(not(test))]
#[tokio::main]
async fn main() {
    let summary = match run(Options::parse()).await {
        Ok(summary) => summary,
        Err(error) => {
            serde_json::json!({"complete":false,"termination":"configuration_or_evidence_error","error":format!("{error:#}")})
        }
    };
    println!("{summary}");
    if summary["complete"] != true {
        std::process::exit(2);
    }
}
