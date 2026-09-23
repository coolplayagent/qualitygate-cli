//! Optional external warning triage. It cannot alter the stored check gate.

use crate::{
    config,
    domain::{
        Artifact, Decision, ExecutionStatus, Report, Severity, Verdict,
        judgment::{
            Assessment, CalibrationStatus, EvaluationContext, JudgmentMode, provider_binding,
        },
    },
    paths, runner, snapshot,
};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::io::AsyncReadExt;

const MAX_REPORT_BYTES: usize = 16 * 1024 * 1024;
const MAX_RESPONSE_BYTES: usize = 64 * 1024;
const MAX_INPUT_BYTES: usize = 1024 * 1024;

pub struct Options {
    pub root: PathBuf,
    pub report: PathBuf,
    pub policy: PathBuf,
    pub output_dir: PathBuf,
    pub fingerprint: Option<String>,
}

pub async fn question_digest(path: PathBuf) -> Result<Value> {
    let bytes = bounded_file(&path, config::judgment::MAX_POLICY_BYTES).await?;
    let digest =
        tokio::task::spawn_blocking(move || config::judgment::question_digest(&bytes)).await??;
    Ok(json!({"source_digest": digest}))
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct JudgmentRun {
    schema_version: u32,
    decision_id: String,
    mode: JudgmentMode,
    original_gate: crate::domain::Gate,
    report_digest: String,
    policy_digest: String,
    repository_digest: String,
    question_digest: String,
    rule_id: String,
    language: String,
    evidence_id: String,
    assessment: Assessment,
    complete: bool,
    route: String,
    calibration_status: Option<CalibrationStatus>,
    calibration: Option<crate::domain::judgment::CalibrationRecord>,
    review_priority_threshold: Option<f64>,
    provider: Option<Artifact>,
    provider_version_digest: Option<String>,
    provider_binding_digest: Option<String>,
    artifacts: Vec<Artifact>,
    audit: Value,
}

async fn bounded_file(path: &Path, limit: usize) -> Result<Vec<u8>> {
    read_bounded_file(path, limit).await.map_err(|error| {
        let missing = error.downcast_ref::<std::io::Error>()
            .is_some_and(|cause| cause.kind() == std::io::ErrorKind::NotFound);
        crate::domain::prerequisites::PrerequisiteIssue::new(
            if missing { crate::domain::prerequisites::FailureCode::InputMissing }
            else { crate::domain::prerequisites::FailureCode::InputUnreadable },
            crate::domain::prerequisites::Phase::Inputs, "Cannot read judgment input")
            .resource(path.display().to_string()).instruction("Supply the selected report/policy input and check its path, permissions and byte budget.").wrap(error)
    })
}

async fn read_bounded_file(path: &Path, limit: usize) -> Result<Vec<u8>> {
    let metadata = tokio::fs::symlink_metadata(path).await?;
    if !metadata.is_file() || metadata.len() > limit as u64 {
        bail!(
            "Input is not a regular file within its byte budget: {}",
            path.display()
        );
    }
    let file = tokio::fs::File::open(path).await?;
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut bytes).await?;
    if bytes.len() > limit {
        bail!("Input changed beyond its byte budget: {}", path.display());
    }
    Ok(bytes)
}

fn artifact(path: &Path, bytes: &[u8]) -> Artifact {
    Artifact {
        path: path.display().to_string(),
        digest: snapshot::digest(bytes),
        bytes: bytes.len() as u64,
    }
}

async fn write_artifact(
    directory: &Path,
    name: &str,
    bytes: &[u8],
    artifacts: &mut Vec<Artifact>,
) -> Result<()> {
    let path = directory.join(name);
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .await?;
    tokio::io::AsyncWriteExt::write_all(&mut file, bytes).await?;
    tokio::io::AsyncWriteExt::flush(&mut file).await?;
    artifacts.push(artifact(&path, bytes));
    Ok(())
}

fn gap(reason: impl std::fmt::Display) -> Assessment {
    Assessment::ExecutionGap {
        reason: reason.to_string().chars().take(1024).collect(),
    }
}

pub async fn run(options: Options) -> Result<(Value, u8)> {
    let report_bytes = bounded_file(&options.report, MAX_REPORT_BYTES)
        .await
        .context("Cannot read bound check report")?;
    let policy_bytes = bounded_file(&options.policy, config::judgment::MAX_POLICY_BYTES)
        .await
        .context("Cannot read judgment policy")?;
    let report: Report =
        serde_json::from_slice(&report_bytes).context("Invalid bound check report")?;
    if report.schema_version != 1
        || report.run_id.is_empty()
        || report.snapshot.content_digest.is_empty()
        || report.policy.config_digest.is_empty()
        || !report.gate.complete
        || report.gate.decision == Decision::Incomplete
    {
        bail!("Check report identity or execution is incomplete");
    }
    let policy = config::judgment::parse(&policy_bytes)?;
    let mut provider_inputs = Vec::new();
    let mut provider_input_bytes = 0;
    for input in &policy.provider_inputs {
        let path = paths::confined(&options.root, Path::new(input))?;
        let bytes = bounded_file(&path, MAX_INPUT_BYTES).await?;
        provider_input_bytes += bytes.len();
        if provider_input_bytes > MAX_INPUT_BYTES {
            bail!("Provider inputs exceed 1 MiB");
        }
        provider_inputs.push(artifact(&path, &bytes));
    }
    let mut selected = Vec::new();
    for check in &report.checks {
        if check.severity != Severity::Warning
            || check.execution.status != ExecutionStatus::Completed
            || check.verdict != Some(Verdict::Fail)
        {
            continue;
        }
        for diagnostic in &check.diagnostics {
            if options
                .fingerprint
                .as_ref()
                .is_none_or(|value| value == &diagnostic.fingerprint)
            {
                selected.push((check, diagnostic));
            }
        }
    }
    if selected.len() != 1 {
        bail!(
            "Warning triage requires one unambiguous completed warning finding; pass --fingerprint when several exist"
        );
    }
    let (check, diagnostic) = selected[0];
    let report_digest = snapshot::digest(&report_bytes);
    let policy_digest = snapshot::digest(&policy_bytes);
    let repository_digest = snapshot::digest(options.root.to_string_lossy().as_bytes());
    let question_digest = policy.question.source_digest.clone();
    let evidence_id = snapshot::digest(&serde_json::to_vec(&(
        &report_digest,
        &check.id,
        &diagnostic.fingerprint,
        &diagnostic.evidence,
    ))?);
    let language = diagnostic.evidence["language"]
        .as_str()
        .unwrap_or("unknown");
    let request = json!({"schema_version":1,"question":policy.question,"report_digest":report_digest,"snapshot_digest":report.snapshot.content_digest,"policy_digest":report.policy.config_digest,"repository_digest":repository_digest,"provider_inputs":provider_inputs,"check_id":check.id,"severity":check.severity,"evidence_id":evidence_id,"finding":{"fingerprint":diagnostic.fingerprint,"file":diagnostic.file,"range":diagnostic.range,"message":diagnostic.message,"evidence":diagnostic.evidence}});
    let request_bytes = serde_json::to_vec(&request)?;
    if request_bytes.len() > MAX_INPUT_BYTES {
        bail!("Judgment request exceeds 1 MiB");
    }
    let started = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let decision_id = snapshot::digest(&serde_json::to_vec(&(
        &report_digest,
        &policy_digest,
        &evidence_id,
        started,
        std::process::id(),
    ))?);
    tokio::fs::create_dir_all(&options.output_dir).await?;
    let directory = options
        .output_dir
        .join(decision_id.strip_prefix("sha256:").unwrap());
    tokio::fs::create_dir(&directory)
        .await
        .context("Cannot create unique judgment artifact directory")?;
    let mut artifacts = Vec::new();
    write_artifact(&directory, "request.json", &request_bytes, &mut artifacts).await?;
    write_artifact(&directory, "policy.yaml", &policy_bytes, &mut artifacts).await?;
    let mut provider = None;
    let mut version_digest = None;
    let mut provider_binding_digest = None;
    let mut raw_response = Vec::new();
    let mut raw_stderr = Vec::new();
    let mut calibration_status = None;
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let outcome = async {
        let executable = runner::identity::executable(&policy.argv[0], &options.root).await?;
        let mut version_argv = policy.version_argv.clone();
        version_argv[0] = executable.path.clone();
        provider = Some(executable.clone());
        let version =
            runner::capture(&version_argv, &options.root, None, Duration::from_secs(10)).await?;
        if version.timed_out
            || version.capture_error.is_some()
            || version.exit_code != Some(0)
            || version.stdout.is_empty()
        {
            bail!("Provider version probe did not complete successfully");
        }
        version_digest = Some(snapshot::digest(&version.stdout));
        provider_binding_digest = Some(provider_binding(
            &executable,
            version_digest.as_deref().unwrap(),
            &provider_inputs,
        )?);
        let mut argv = policy.argv.clone();
        argv[0] = executable.path.clone();
        let output = runner::capture(
            &argv,
            &options.root,
            Some(request_bytes.clone()),
            Duration::from_secs(policy.timeout_seconds.into()),
        )
        .await?;
        raw_response = output.stdout;
        raw_stderr = output.stderr;
        if output.timed_out {
            bail!("Provider timed out");
        }
        if let Some(error) = output.capture_error {
            bail!("Provider output capture failed: {error}");
        }
        if output.exit_code != Some(0) {
            bail!(
                "Provider exited without a complete assessment: {:?}",
                output.exit_code
            );
        }
        if raw_response.len() > MAX_RESPONSE_BYTES {
            bail!("Provider response exceeds 64 KiB");
        }
        if runner::identity::executable(&policy.argv[0], &options.root)
            .await?
            .digest
            != executable.digest
        {
            bail!("Provider executable changed during judgment");
        }
        if snapshot::digest(&bounded_file(&options.report, MAX_REPORT_BYTES).await?)
            != report_digest
            || snapshot::digest(
                &bounded_file(&options.policy, config::judgment::MAX_POLICY_BYTES).await?,
            ) != policy_digest
        {
            bail!("Bound report or judgment policy changed during execution");
        }
        for input in &provider_inputs {
            if snapshot::digest(&bounded_file(Path::new(&input.path), MAX_INPUT_BYTES).await?)
                != input.digest
            {
                bail!("Provider input changed during execution: {}", input.path);
            }
        }
        let assessment: Assessment = serde_json::from_slice(&raw_response)
            .context("Provider response is not a typed assessment")?;
        calibration_status = assessment.validate(
            &policy,
            &EvaluationContext {
                evidence_id: &evidence_id,
                provider_digest: provider_binding_digest.as_deref().unwrap(),
                repository_digest: &repository_digest,
                language,
                rule_id: &check.id,
                now_unix: now,
            },
        )?;
        Ok::<_, anyhow::Error>(assessment)
    }
    .await;
    let assessment = match outcome {
        Ok(value) => value,
        Err(error) => gap(format!("{error:#}")),
    };
    write_artifact(&directory, "response.json", &raw_response, &mut artifacts).await?;
    write_artifact(&directory, "stderr.txt", &raw_stderr, &mut artifacts).await?;
    let complete = !matches!(assessment, Assessment::ExecutionGap { .. });
    let route = if !complete {
        "inspect_gap"
    } else if policy.mode == JudgmentMode::Shadow {
        "shadow_only"
    } else if let Assessment::Probabilistic { distribution, .. } = &assessment {
        if policy.review_priority_threshold.is_some_and(|threshold| {
            distribution
                .get(&policy.question.positive_choice)
                .is_some_and(|value| *value >= threshold)
        }) {
            "prioritize_review"
        } else {
            "manual_review"
        }
    } else {
        "manual_review"
    };
    let original_gate = report.gate.clone();
    let run = JudgmentRun {
        schema_version: 1,
        decision_id,
        mode: policy.mode,
        original_gate,
        report_digest,
        policy_digest,
        repository_digest,
        question_digest,
        evidence_id,
        assessment,
        complete,
        route: route.into(),
        calibration_status,
        calibration: policy.calibration.clone(),
        review_priority_threshold: policy.review_priority_threshold,
        rule_id: check.id.clone(),
        language: language.into(),
        provider,
        provider_version_digest: version_digest,
        provider_binding_digest,
        artifacts,
        audit: json!({"raw_response":"response.json","request":"request.json","stderr":"stderr.txt","report_path":options.report,"provider_inputs":provider_inputs,"provider_replay":"retained_response_only","automatic_gate_change":false,"model_or_question_change_requires_new_calibration":true,"warning_findings_retained":true,"started_unix_nanos":started}),
    };
    let value = serde_json::to_value(&run)?;
    let result_bytes = serde_json::to_vec_pretty(&value)?;
    write_artifact(&directory, "decision.json", &result_bytes, &mut Vec::new()).await?;
    Ok((
        value,
        if complete {
            0
        } else {
            Decision::Incomplete.exit_code()
        },
    ))
}

mod pilot;
pub use pilot::{Options as PilotOptions, summarize};
