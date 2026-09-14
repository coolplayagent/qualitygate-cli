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
    #[command(hide = true)]
    SelfcheckProbe {
        #[arg(value_parser = ["success", "failure", "timeout", "overflow"])]
        mode: String,
    },
    /// Discover the repository and create a candidate policy without overwriting.
    Init {
        /// Include structurally complete command suggestions in a new candidate.
        #[arg(long)]
        with_checks: bool,
    },
    /// Execute policy and acceptance checks against a selected Git snapshot.
    Check(Box<CheckArgs>),
    /// Compare bundled minimal/typical/stress fixtures with independent goldens.
    Selfcheck {
        #[arg(long, value_parser = ["minimal", "typical", "stress"])]
        fixture: Option<String>,
        #[arg(long)]
        rule: Option<String>,
    },
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
    /// Inventory all available rule packages, without enabling them.
    List {
        #[arg(long)]
        language: Option<String>,
        #[arg(long, default_value = "all", value_parser = ["all", "builtin", "project"])]
        source: String,
    },
    Enable {
        rule_id: String,
    },
    /// Export the shipped project-rule JSON Schema (always JSON).
    Schema,
    /// Validate YAML/JSON rules, DSL semantics and current source bindings.
    Validate {
        #[arg(default_value = config::catalog::PROJECT_RULES_DIR)]
        path: String,
    },
    /// Compute a source binding from one exact normative ATX section.
    Source {
        #[arg(long)]
        document: String,
        #[arg(long)]
        section: String,
    },
    /// Validate and publish a candidate to qualitygate/rules/<id>.yaml.
    Generate {
        #[arg(long)]
        input: String,
    },
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
    #[arg(long, group = "selection")]
    mr: Option<String>,
    #[arg(long, requires = "mr")]
    mr_api_base: Option<String>,
    #[arg(long, conflicts_with_all = ["diff", "staged", "mr"])]
    base: Option<String>,
    #[arg(long, default_value = "full", value_parser = ["quick", "full"])]
    profile: String,
    #[arg(long)]
    task: Option<String>,
    #[arg(long)]
    policy_ref: Option<String>,
    #[arg(long)]
    output_dir: Option<PathBuf>,
    /// Caller-controlled JSON trust store outside the repository.
    #[arg(long, requires = "evidence_dir")]
    trust_store: Option<PathBuf>,
    /// Directory containing signed acceptance records, outside the repository.
    #[arg(long, requires = "trust_store")]
    evidence_dir: Option<PathBuf>,
    #[arg(long, value_enum)]
    severity: Option<Severity>,
}

impl Cli {
    pub async fn run(self) -> Result<(String, u8)> {
        if let Command::SelfcheckProbe { mode } = self.command {
            return Ok(application::selfcheck::probe(&mode).await);
        }
        if let Command::Selfcheck { fixture, rule } = self.command {
            let report = application::selfcheck::run(fixture, rule).await;
            let code = report.decision.exit_code();
            return Ok((super::render::selfcheck(&report, self.format)?, code));
        }
        if let Command::Rules {
            command: Rules::Schema,
        } = self.command
        {
            let schema = tokio::task::spawn_blocking(config::rule_schema::document).await??;
            return Ok((serde_json::to_string_pretty(&schema)?, 0));
        }
        let root = self
            .root
            .canonicalize()
            .context("Repository root does not exist")?;
        match self.command {
            Command::SelfcheckProbe { .. } => {
                unreachable!("fixed probe handled before repository discovery")
            }
            Command::Selfcheck { .. } => {
                unreachable!("selfcheck is independent of repository inputs")
            }
            Command::Init { with_checks } => {
                let path = self.config.clone();
                let initialized = tokio::task::spawn_blocking(move || {
                    config::initialize_at(&root, path.as_ref(), with_checks)
                })
                .await??;
                Ok((
                    super::render::metadata(
                        &serde_json::json!({"schema_version":1,"source":self.config,"config":initialized.config,"discovery":initialized.discovery,"created":initialized.created,"with_checks_applied":initialized.with_checks_applied,"status":"candidate","message":"Review candidate checks, report requirements and capability gaps before enforcing this policy"}),
                        self.format,
                    )?,
                    0,
                ))
            }
            Command::Config { .. } => {
                let path = self.config.clone();
                let (config, catalog) = tokio::task::spawn_blocking(move || -> Result<_> {
                    let config = config::read(&root, path.as_ref())?;
                    let catalog = config::catalog::read(&root, &config)?;
                    Ok((catalog.resolve(&config)?, catalog))
                })
                .await??;
                Ok((
                    super::render::metadata(
                        &serde_json::json!({"schema_version":1,"source": self.config, "config":config,"catalog":catalog}),
                        self.format,
                    )?,
                    0,
                ))
            }
            Command::Rules { command } => run_rules(root, self.config, command, self.format).await,
            Command::Check(args) => {
                let selection = if let Some(url) = args.mr {
                    Selection::MergeRequest {
                        url,
                        api_base: args.mr_api_base,
                    }
                } else if let Some(diff) = args.diff {
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
                    trust_store: args.trust_store,
                    evidence_dir: args.evidence_dir,
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

async fn run_rules(
    root: PathBuf,
    path: String,
    command: Rules,
    format: Format,
) -> Result<(String, u8)> {
    let (value, code) = tokio::task::spawn_blocking(move || -> Result<(serde_json::Value, u8)> {
        match command {
            Rules::List { language, source } => Ok((config::rule_query::list(&root, &path, language.as_deref(), &source)?, 0)),
            Rules::Enable { rule_id } => {
                config::enable_rule(&root, path.as_ref(), &rule_id)?;
                Ok((serde_json::json!({"schema_version":1,"enabled":rule_id,"status":"candidate"}), 0))
            }
            Rules::Schema => unreachable!("schema export does not require a repository"),
            Rules::Validate { path } => {
                let report = config::rule_authoring::validate(&root, &path)?;
                let code = report.decision.exit_code();
                Ok((serde_json::to_value(report)?, code))
            }
            Rules::Source { document, section } => Ok((serde_json::json!({"schema_version":1,"source":config::rule_authoring::source(&root, &document, &section)?,"review_trust":"local_candidate"}), 0)),
            Rules::Generate { input } => Ok((config::rule_authoring::generate(&root, &input)?, 0)),
        }
    }).await??;
    Ok((super::render::metadata(&value, format)?, code))
}
