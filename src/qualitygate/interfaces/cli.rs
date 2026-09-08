use crate::{
    application::{self, CheckOptions},
    config,
    domain::Severity,
    snapshot::Selection,
};
use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
pub enum Format {
    Json,
    #[default]
    Table,
    Markdown,
}

#[derive(Debug, Parser)]
#[command(
    name = "qualitygate",
    version,
    about = "Snapshot-bound repository policy and task acceptance gates"
)]
pub struct Cli {
    #[arg(long, global = true, default_value = ".")]
    root: PathBuf,
    #[arg(long, global = true, default_value = config::CONFIG_FILE)]
    config: String,
    #[arg(long, global = true, value_enum, default_value = "table")]
    format: Format,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Discover the repository and create a candidate policy without overwriting.
    Init,
    /// Execute policy and acceptance checks against a selected Git snapshot.
    Check(CheckArgs),
    /// Inspect available rules or enable a local candidate rule.
    Rules {
        #[command(subcommand)]
        command: Rules,
    },
    /// Display effective configuration and its source.
    Config {
        #[arg(long, required = true)]
        show: bool,
    },
}

#[derive(Debug, Subcommand)]
enum Rules {
    List,
    Enable { rule_id: String },
}

#[derive(Debug, Args)]
#[group(skip)]
struct CheckArgs {
    #[arg(long, group = "selection")]
    diff: Option<String>,
    #[arg(long, group = "selection")]
    staged: bool,
    #[arg(long, group = "selection")]
    worktree: bool,
    #[arg(long, group = "selection")]
    path: Option<String>,
    #[arg(long, conflicts_with_all = ["diff", "staged"])]
    base: Option<String>,
    #[arg(long, default_value = "full", value_parser = ["quick", "full"])]
    profile: String,
    #[arg(long)]
    task: Option<String>,
    #[arg(long)]
    policy_ref: Option<String>,
    #[arg(long)]
    output_dir: Option<PathBuf>,
    #[arg(long, value_enum)]
    severity: Option<Severity>,
}

impl Cli {
    pub async fn run(self) -> Result<(String, u8)> {
        let root = self
            .root
            .canonicalize()
            .context("Repository root does not exist")?;
        match self.command {
            Command::Init => {
                let config = tokio::task::spawn_blocking(move || config::init(&root)).await??;
                Ok((
                    serde_json::to_string_pretty(
                        &serde_json::json!({"schema_version":1,"config":config,"status":"candidate","message":"Review the discovered candidate policy before enforcing it"}),
                    )?,
                    0,
                ))
            }
            Command::Config { .. } => {
                let path = self.config.clone();
                let config =
                    tokio::task::spawn_blocking(move || config::read(&root, path.as_ref()))
                        .await??;
                Ok((
                    serde_json::to_string_pretty(
                        &serde_json::json!({"source": self.config, "config":config}),
                    )?,
                    0,
                ))
            }
            Command::Rules { command } => run_rules(root, self.config, command).await,
            Command::Check(args) => {
                let selection = if let Some(diff) = args.diff {
                    let (base, head) = diff
                        .split_once("..")
                        .context("--diff requires <base>..<head>")?;
                    if base.is_empty() || head.is_empty() || head.starts_with('.') {
                        bail!("--diff requires two explicit commit endpoints");
                    }
                    Selection::Diff {
                        base: base.into(),
                        head: head.into(),
                    }
                } else if args.staged {
                    Selection::Staged
                } else if let Some(path) = args.path {
                    Selection::Path {
                        path,
                        base: args.base.unwrap_or_else(|| "HEAD".into()),
                    }
                } else {
                    Selection::Worktree {
                        base: args.base.unwrap_or_else(|| "HEAD".into()),
                    }
                };
                let options = CheckOptions {
                    root,
                    config: self.config,
                    selection,
                    profile: args.profile,
                    task: args.task,
                    policy_ref: args.policy_ref,
                    output_dir: args.output_dir,
                };
                let report = application::check(options).await?;
                let code = report.gate.decision.exit_code();
                Ok((
                    super::render::report(&report, self.format, args.severity)?,
                    code,
                ))
            }
        }
    }
}

async fn run_rules(root: PathBuf, path: String, command: Rules) -> Result<(String, u8)> {
    let value = tokio::task::spawn_blocking(move || {
        let mut config = config::read(&root, path.as_ref())?;
        match command {
            Rules::List => {
                let rules: Vec<_> = crate::adapters::rules::BUILTINS.iter().map(|(id, description)| serde_json::json!({"id":id,"description":description,"configuration":config.rules.get(*id)})).collect();
                Ok(serde_json::json!({"rules":rules}))
            }
            Rules::Enable { rule_id } => {
                if !crate::adapters::rules::BUILTINS.iter().any(|(id, _)| *id == rule_id) { bail!("Unknown rule: {rule_id}"); }
                config.rules.entry(rule_id.clone()).or_default().enabled = true;
                for profile in config.profiles.values_mut() { if !profile.include.contains(&rule_id) { profile.include.push(rule_id.clone()); } }
                let data = serde_norway::to_string(&config)?;
                config::parse(data.as_bytes())?;
                std::fs::write(crate::paths::confined(&root, path.as_ref())?, data)?;
                Ok(serde_json::json!({"enabled":rule_id,"status":"candidate"}))
            }
        }
    }).await??;
    Ok((serde_json::to_string_pretty(&value)?, 0))
}
