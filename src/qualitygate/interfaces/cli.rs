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
    /// Read-only pilot evidence and verification of externally signed start authorization.
    Pilot {
        #[command(subcommand)]
        command: super::pilot::Pilot,
    },
    /// Evidence retention, candidate revisions and immutable policy history.
    Policy {
        #[command(subcommand)]
        command: super::policy::Policy,
    },
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
    /// Discover rules and manage validated candidate configuration.
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
    #[command(alias = "evidence")]
    History {
        rule_id: String,
        #[arg(long, default_value_t = 0)]
        offset: usize,
        #[arg(long, default_value_t = 32, value_parser = clap::value_parser!(u16).range(1..=256))]
        limit: u16,
    },
    /// Prepare a fresh candidate for protected revalidation; approval is still required.
    Revalidate(super::rule_lifecycle::Change),
    Demote(super::rule_lifecycle::Change),
    Deprecate(super::rule_lifecycle::Change),
    Retire(super::rule_lifecycle::Change),
    Revoke(super::rule_lifecycle::Change),
    /// Load selected categories together with all mandatory policy constraints.
    Context {
        #[arg(long)]
        category: Option<String>,
        #[arg(long)]
        policy_ref: Option<String>,
    },
    /// Inventory all available rule packages, without enabling them.
    List {
        #[arg(long)]
        language: Option<String>,
        #[arg(long, default_value = "all", value_parser = ["all", "builtin", "project"])]
        source: String,
        #[arg(long)]
        category: Option<String>,
    },
    Enable {
        rule_id: String,
    },
    Disable {
        rule_id: String,
    },
    Describe {
        rule_id: String,
    },
    Assign {
        rule_id: String,
        #[arg(long)]
        category: String,
    },
    Unassign {
        rule_id: String,
        #[arg(long)]
        category: String,
    },
    Configure {
        rule_id: String,
        /// JSON values, for example --param 'patterns.rust="^test_"'.
        #[arg(long = "param")]
        parameters: Vec<String>,
        #[arg(long, value_enum)]
        severity: Option<Severity>,
        #[arg(long, action = clap::ArgAction::Set)]
        required: Option<bool>,
    },
    Categories {
        #[command(subcommand)]
        command: Option<Categories>,
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

#[derive(Debug, Subcommand)]
enum Categories {
    List,
    Create {
        name: String,
        #[arg(long, default_value = "")]
        description: String,
    },
    Rename {
        name: String,
        new_name: String,
    },
    Delete {
        name: String,
        #[arg(long)]
        force: bool,
    },
}

#[derive(Debug, Args)]
#[group(skip)]
struct CheckArgs {
    /// Emit bounded JSON feedback linked to the unfiltered persisted report.
    #[arg(long)]
    feedback: bool,
    /// Feedback byte limit including the trailing newline (4..=256 KiB).
    #[arg(long, requires = "feedback", value_parser = clap::value_parser!(u32).range(4096..=262144))]
    feedback_max_bytes: Option<u32>,
    #[arg(long, group = "selection")]
    diff: Option<String>,
    #[arg(long, group = "selection")]
    staged: bool,
    #[arg(long, group = "selection")]
    worktree: bool,
    /// Restrict feedback to a file or directory within the selected snapshot.
    #[arg(long)]
    path: Option<String>,
    #[arg(long, group = "selection")]
    mr: Option<String>,
    #[arg(long, requires = "mr")]
    mr_api_base: Option<String>,
    #[arg(long, conflicts_with_all = ["diff", "staged", "mr"])]
    base: Option<String>,
    /// Reject a replay if its captured base differs from this full commit ID.
    #[arg(long)]
    expect_base: Option<String>,
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
    /// Maximum total content bytes per snapshot, in MiB (1..=1024).
    #[arg(long, default_value_t = 256, value_parser = clap::value_parser!(u32).range(1..=1024))]
    snapshot_max_mib: u32,
    /// Shared maximum concurrent snapshot content readers (1..=16).
    #[arg(long, default_value_t = 4, value_parser = clap::value_parser!(u32).range(1..=16))]
    snapshot_jobs: u32,
    /// Deadline for each complete snapshot acquisition (1..=3600 seconds).
    #[arg(long, default_value_t = 120, value_parser = clap::value_parser!(u32).range(1..=3600))]
    snapshot_timeout_secs: u32,
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
                let value =
                    application::policy_active::configuration(root, self.config.clone()).await?;
                Ok((super::render::metadata(&value, self.format)?, 0))
            }
            Command::Rules { command } => run_rules(root, self.config, command, self.format).await,
            Command::Pilot { command } => {
                let resolve = |input: PathBuf| {
                    if input.is_absolute() {
                        input
                    } else {
                        root.join(input)
                    }
                };
                let (value, code) = match command {
                    super::pilot::Pilot::Seal { input } => {
                        application::pilot::seal(resolve(input)).await?
                    }
                    super::pilot::Pilot::AuthorizationSubject { input } => {
                        application::pilot::authorization_subject(resolve(input)).await?
                    }
                    super::pilot::Pilot::AcceptanceSubject {
                        input,
                        trust_store,
                        authorization,
                    } => {
                        application::pilot::acceptance_subject(
                            root.clone(),
                            resolve(input),
                            trust_store,
                            authorization,
                        )
                        .await?
                    }
                    super::pilot::Pilot::Summarize {
                        input,
                        trust_store,
                        authorization,
                        acceptance,
                    } => {
                        application::pilot::summarize(
                            root.clone(),
                            resolve(input),
                            trust_store,
                            authorization,
                            acceptance,
                        )
                        .await?
                    }
                };
                Ok((super::render::metadata(&value, self.format)?, code))
            }
            Command::Policy { command } => {
                let (value, code) = super::policy::run(root, self.config, command).await?;
                Ok((super::render::metadata(&value, self.format)?, code))
            }
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
                } else if args.worktree {
                    Selection::Worktree {
                        base: args.base.unwrap_or_else(|| "HEAD".into()),
                    }
                } else if let Some(path) = &args.path {
                    Selection::Path {
                        path: path.clone(),
                        base: args.base.unwrap_or_else(|| "HEAD".into()),
                    }
                } else {
                    Selection::Worktree {
                        base: args.base.unwrap_or_else(|| "HEAD".into()),
                    }
                };
                let path_filter = if matches!(selection, Selection::Path { .. }) {
                    None
                } else {
                    args.path
                };
                let options = CheckOptions {
                    root,
                    config: self.config,
                    selection,
                    snapshot_options: crate::snapshot::CaptureOptions {
                        max_bytes: args.snapshot_max_mib as usize * 1024 * 1024,
                        jobs: args.snapshot_jobs as usize,
                        timeout: std::time::Duration::from_secs(args.snapshot_timeout_secs.into()),
                        path_filter,
                    },
                    profile: args.profile,
                    task: args.task,
                    policy_ref: args.policy_ref,
                    output_dir: args.output_dir,
                    trust_store: args.trust_store,
                    evidence_dir: args.evidence_dir,
                };
                let report =
                    application::check_with_expected_base(options, args.expect_base.as_deref())
                        .await?;
                let code = report.gate.decision.exit_code();
                if args.feedback {
                    return Ok((
                        application::feedback::render(
                            report,
                            args.feedback_max_bytes.unwrap_or(32768) as usize,
                            args.severity,
                        )
                        .await?,
                        code,
                    ));
                }
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
    use crate::domain::rule_lifecycle::RuleState;
    let lifecycle = match &command {
        Rules::Revalidate(_) => Some(RuleState::Revalidate),
        Rules::Demote(_) => Some(RuleState::Demoted),
        Rules::Deprecate(_) => Some(RuleState::Deprecated),
        Rules::Retire(_) => Some(RuleState::Retired),
        Rules::Revoke(_) => Some(RuleState::Revoked),
        _ => None,
    };
    if let Some(state) = lifecycle {
        let (Rules::Revalidate(change)
        | Rules::Demote(change)
        | Rules::Deprecate(change)
        | Rules::Retire(change)
        | Rules::Revoke(change)) = command
        else {
            unreachable!("selected lifecycle operation")
        };
        let result = super::rule_lifecycle::run(root, change, state).await?;
        return Ok((super::render::metadata(&result, format)?, 0));
    }
    if let Rules::Context {
        category,
        policy_ref,
    } = command
    {
        let value = application::rule_context::read(root, path, category, policy_ref).await?;
        return Ok((super::render::metadata(&value, format)?, 0));
    }
    let active = application::policy_active::load(root.clone()).await?;
    let (mut value, code) = tokio::task::spawn_blocking(move || -> Result<(serde_json::Value, u8)> {
        match command {
            Rules::History { rule_id, offset, limit } => Ok((config::policy_effectiveness::rule_history(&root, &rule_id, offset, limit.into())?, 0)),
            Rules::Revalidate(_) | Rules::Demote(_) | Rules::Deprecate(_) | Rules::Retire(_) | Rules::Revoke(_) => unreachable!("lifecycle transitions are dispatched above"),
            Rules::Context { .. } => unreachable!("context loads its selected policy asynchronously"),
            Rules::List { language, source, category } => Ok((config::rule_query::list_filtered(&root, &path, language.as_deref(), &source, category.as_deref())?, 0)),
            Rules::Enable { rule_id } => {
                let mut result = config::rule_management::update(&root, path.as_ref(), config::rule_management::Mutation::Enable(rule_id.clone()))?;
                result["enabled"] = serde_json::json!(rule_id);
                Ok((result, 0))
            }
            Rules::Disable { rule_id } => Ok((config::rule_management::update(&root, path.as_ref(), config::rule_management::Mutation::Disable(rule_id))?, 0)),
            Rules::Describe { rule_id } => Ok((config::rule_query::describe(&root, &path, &rule_id)?, 0)),
            Rules::Assign { rule_id, category } => Ok((config::rule_management::update(&root, path.as_ref(), config::rule_management::Mutation::Assign { id: rule_id, category })?, 0)),
            Rules::Unassign { rule_id, category } => Ok((config::rule_management::update(&root, path.as_ref(), config::rule_management::Mutation::Unassign { id: rule_id, category })?, 0)),
            Rules::Configure { rule_id, parameters, severity, required } => Ok((config::rule_management::update(&root, path.as_ref(), config::rule_management::Mutation::Configure { id: rule_id, parameters, severity, required })?, 0)),
            Rules::Categories { command: None | Some(Categories::List) } => Ok((config::rule_query::categories(&root, &path)?, 0)),
            Rules::Categories { command: Some(command) } => {
                use config::rule_management::Mutation;
                let mutation = match command {
                    Categories::List => unreachable!("category list is read-only"),
                    Categories::Create { name, description } => Mutation::Create { name, description },
                    Categories::Rename { name, new_name } => Mutation::Rename { name, new_name },
                    Categories::Delete { name, force } => Mutation::Delete { name, force },
                };
                Ok((config::rule_management::update(&root, path.as_ref(), mutation)?, 0))
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
    if active.is_some() && value.get("review_trust").is_some() {
        value["review_trust"] = serde_json::json!("signed_active_policy");
    }
    Ok((super::render::metadata(&value, format)?, code))
}
