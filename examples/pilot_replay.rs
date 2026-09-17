//! Offline adapter for retained phase-B module experiments. Never invokes an Agent.
use anyhow::{Context, Result, bail};
use clap::Parser;
use qualitygate::{
    domain::{Artifact, Report, pilot::*},
    snapshot::digest,
};
use serde_json::{Value, json};
use std::{
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Parser)]
struct Options {
    /// Existing phase-B index.json; repeat to retain different sandbox runs.
    #[arg(long, required = true)]
    index: Vec<PathBuf>,
    /// New output directory. Existing directories are never overwritten.
    #[arg(long)]
    output: PathBuf,
}
fn read(path: &Path, max: u64) -> Result<Vec<u8>> {
    if !std::fs::symlink_metadata(path)?.is_file() {
        bail!("Expected regular retained evidence");
    }
    let file = std::fs::File::open(path)?;
    if !file.metadata()?.is_file() {
        bail!("Evidence changed type");
    }
    let mut bytes = Vec::new();
    file.take(max + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max {
        bail!("Retained evidence exceeds its limit");
    }
    Ok(bytes)
}
fn artifact(root: &Path, ledger: &Path, value: &Value, total: &mut u64) -> Result<Vec<u8>> {
    let a: Artifact = serde_json::from_value(value.clone())?;
    if a.bytes > 16 * 1024 * 1024 {
        bail!("Artifact exceeds 16 MiB");
    }
    *total = total.saturating_add(a.bytes + 1);
    if *total > 64 * 1024 * 1024 {
        bail!("Import exceeds 64 MiB");
    }
    let path = Path::new(&a.path);
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        ledger.join(path)
    };
    let confined = qualitygate::paths::confined(
        root,
        path.strip_prefix(root)
            .context("Artifact is outside its selected archive")?,
    )?;
    let bytes = read(&confined, a.bytes)?;
    if bytes.len() as u64 != a.bytes || digest(&bytes) != a.digest {
        bail!("Retained artifact digest or size mismatch");
    }
    Ok(bytes)
}
fn verify_tree(root: &Path, ledger: &Path, value: &Value, total: &mut u64) -> Result<()> {
    match value {
        Value::Object(fields)
            if fields.contains_key("path")
                && fields.contains_key("digest")
                && fields.contains_key("bytes") =>
        {
            artifact(root, ledger, value, total)?;
        }
        Value::Object(fields) => {
            for v in fields.values() {
                verify_tree(root, ledger, v, total)?;
            }
        }
        Value::Array(values) => {
            for v in values {
                verify_tree(root, ledger, v, total)?;
            }
        }
        _ => {}
    }
    Ok(())
}
fn text<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    v[key].as_str().with_context(|| format!("Missing {key}"))
}
fn main() -> Result<()> {
    let o = Options::parse();
    if o.index.len() > 4 {
        bail!("At most four retained experiment indexes");
    }
    std::fs::create_dir_all(
        o.output
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or(Path::new(".")),
    )?;
    std::fs::create_dir(&o.output).context("Output directory must be new")?;
    let mut manifest: Manifest =
        serde_json::from_str(include_str!("../templates/pilot/observation-v1.json"))?;
    manifest.id = "phase-b-offline-replay".into();
    manifest.protocol.sampling="Retrospective replay of all records from explicitly selected extracted-module experiments; not the sealed business pilot".into();
    let mut total = 0;
    let mut sources = Vec::new();
    for (batch, path) in o.index.iter().enumerate() {
        let root = dunce::canonicalize(path.parent().context("Index requires parent")?)?;
        let path = qualitygate::paths::confined(
            &root,
            Path::new(path.file_name().context("Index requires filename")?),
        )?;
        let bytes = read(&path, 1024 * 1024)?;
        let index: Value = serde_json::from_slice(&bytes)?;
        if index["schema_version"] != 1
            || index["business_trial"] != false
            || index["origin"] != "live_codex_on_extracted_module"
        {
            bail!("Only retained phase-B module experiments are supported");
        }
        let records = index["records"].as_array().context("Missing records")?;
        if records.len() > 16 {
            bail!("Too many replay records");
        }
        sources.push(json!({"path":path,"digest":digest(&bytes),"records":records.len()}));
        std::fs::write(o.output.join(format!("source-{batch}.json")), bytes)?;
        for (i, record) in records.iter().enumerate() {
            let ledger_path = qualitygate::paths::confined(
                &root,
                Path::new(text(record, "index_path")?).strip_prefix(&root)?,
            )?;
            let ledger_dir = ledger_path.parent().context("Ledger requires parent")?;
            let bytes = read(&ledger_path, 1024 * 1024)?;
            if digest(&bytes) != text(record, "index_digest")? {
                bail!("Ledger digest changed");
            }
            let ledger: Value = serde_json::from_slice(&bytes)?;
            if ledger["schema_version"] != 1
                || ledger["budget"]["attempts"] != 3
                || ledger["budget"]["seconds"] != 1800
            {
                bail!("Unexpected harness budget or schema");
            }
            verify_tree(&root, ledger_dir, &ledger, &mut total)?;
            let initial: Report = serde_json::from_slice(&artifact(
                &root,
                ledger_dir,
                &ledger["initial"]["full_report"],
                &mut total,
            )?)?;
            let id = format!("batch-{batch}-{}", text(record, "case")?);
            let attempts = ledger["attempts"].as_array().context("Missing attempts")?;
            if attempts.len() > 10 {
                bail!("Too many attempts");
            }
            let permissions = if ledger["replacement_file"].is_string() {
                "read-only proposal, confined source replacement"
            } else {
                "workspace-write direct execution"
            };
            manifest.assignments.push(Assignment {
                id: id.clone(),
                input_id: id.clone(),
                task_id: initial.plan.task_id.clone().context("Missing task")?,
                task_kind: match text(record, "kind")? {
                    "bug-fix" => TaskKind::BugFix,
                    "refactor" => TaskKind::Refactor,
                    _ => bail!("Unsupported experiment task"),
                },
                origin: Origin::ExtractedModule,
                cohort: Cohort {
                    agent_version: text(record, "agent_version")?.into(),
                    harness_digest: text(&ledger, "harness_source_digest")?.into(),
                    requested_model: text(record, "requested_model")?.into(),
                    actual_model: record["actual_model"].as_str().map(str::to_owned),
                    reasoning_effort: text(record, "reasoning_effort")?.into(),
                    workflow: Workflow::Qualitygate,
                    environment_digest: initial.environment_digest.clone(),
                    tools_digest: tool_inventory_digest(&initial),
                    cache: "provider cache uncontrolled".into(),
                    permissions: permissions.into(),
                },
                base: initial.snapshot.base.clone(),
                initial_snapshot: initial.snapshot.content_digest.clone(),
                initial_report: None,
                config_digest: initial.policy.config_digest.clone(),
                task_digest: initial
                    .policy
                    .task_contract_digest
                    .clone()
                    .context("Missing task digest")?,
                required_checks: initial.plan.required_checks.clone(),
                expected_issues: None,
                eligible_repair: true,
                exclusion: None,
            });
            let mut observed_at = 0;
            let mut observations = Vec::new();
            for (j, attempt) in attempts.iter().enumerate() {
                let check = &attempt["recheck"];
                let (report, snapshot) = if check["full_report"].is_object() {
                    let bytes = artifact(&root, ledger_dir, &check["full_report"], &mut total)?;
                    let report: Report = serde_json::from_slice(&bytes)?;
                    observed_at = observed_at.max(
                        report
                            .checks
                            .iter()
                            .filter_map(|c| c.execution.ended_at_ms)
                            .max()
                            .unwrap_or(0)
                            / 1000,
                    );
                    let name = format!("report-{batch}-{i}-{j}.json");
                    std::fs::write(o.output.join(&name), &bytes)?;
                    (
                        Some(Artifact {
                            path: name,
                            digest: digest(&bytes),
                            bytes: bytes.len() as u64,
                        }),
                        report.snapshot.content_digest,
                    )
                } else {
                    (None, initial.snapshot.content_digest.clone())
                };
                let agent = &attempt["agent"];
                let status = if agent["timed_out"] == true {
                    AttemptStatus::TimedOut
                } else if agent["exit_code"] != 0 || !agent["capture_error"].is_null() {
                    AttemptStatus::Failed
                } else if report.is_some() {
                    AttemptStatus::Completed
                } else {
                    AttemptStatus::Incomplete
                };
                let check_ms = check["duration_ms"].as_u64();
                let elapsed_ms = agent["duration_ms"]
                    .as_u64()
                    .context("Missing measured agent duration")?
                    + check_ms.unwrap_or(0)
                    + if j == 0 {
                        ledger["initial"]["duration_ms"]
                            .as_u64()
                            .context("Missing initial check duration")?
                    } else {
                        0
                    };
                observations.push(Attempt {
                    number: (j + 1) as u16,
                    status,
                    elapsed_ms,
                    check_elapsed_ms: check_ms,
                    profile: "full".into(),
                    snapshot_digest: snapshot,
                    report,
                    cost: None,
                    usage: if agent["usage"].is_object() {
                        Some(serde_json::from_value(agent["usage"].clone())?)
                    } else {
                        None
                    },
                });
            }
            manifest.observations.push(Observation {
                start_sequence: None,
                model_evidence: None,
                assignment_id: id,
                observed_at,
                attempts: observations,
                findings: vec![],
                review_active_ms: None,
                review_comments: None,
                rework_rounds: None,
            });
            std::fs::write(o.output.join(format!("ledger-{batch}-{i}.json")), bytes)?;
        }
    }
    validate(&manifest)?;
    let path = o.output.join("observations.json");
    std::fs::write(&path, serde_json::to_vec_pretty(&manifest)?)?;
    let (loaded, reports) = qualitygate::config::pilot::load(&path)?;
    let result = summarize(&loaded, &reports, qualitygate::config::policy_store::now()?)?;
    std::fs::write(
        o.output.join("summary.json"),
        serde_json::to_vec_pretty(&result)?,
    )?;
    std::fs::write(
        o.output.join("import.json"),
        serde_json::to_vec_pretty(&json!({"sources":sources,
        "authority":"retrospective_engineering_observation","manifest_digest":digest(&serde_json::to_vec_pretty(&manifest)?),
        "known_limits":["All selected historical assignments retained; these are not eight production tasks",
            "Original Git snapshots differ across module runs, so inputs cannot establish paired causal model/workflow benefit",
            "Durations include recorded agent and recheck wall time, plus the initial check in the first attempt; orchestration overhead was not separately measured",
            "Budget enforcement comes from the original bounded harness; no new models or checks were executed",
            "No invented monetary cost, issue review, model identity or trial seal"]}))?,
    )?;
    println!("{}", o.output.join("summary.json").display());
    Ok(())
}
