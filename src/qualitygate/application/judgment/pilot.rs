//! Snapshot-bound pilot summaries from retained judgment runs and independent labels.

use super::JudgmentRun;
use crate::{
    domain::judgment::{Assessment, CalibrationStatus, JudgmentMode},
    snapshot,
};
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_RUNS: usize = 256;
const MAX_TOTAL_BYTES: usize = 64 * 1024 * 1024;

pub struct Options {
    pub runs: PathBuf,
    pub labels: PathBuf,
    pub audit_seed: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Label {
    decision_id: String,
    finding_valid: bool,
    reviewer: String,
    source_digest: String,
    reviewed_unix: u64,
    independent: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Labels {
    schema_version: u32,
    repository_digest: String,
    question_digest: String,
    provider_binding_digest: String,
    model_digest: String,
    calibrator_digest: String,
    label_source_digest: String,
    target_event: String,
    validation_from_unix: u64,
    validation_until_unix: u64,
    audit_fraction: f64,
    labels: Vec<Label>,
}

fn bounded(path: &Path, max: usize) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.len() > max as u64 {
        bail!(
            "Pilot input is not a bounded regular file: {}",
            path.display()
        );
    }
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(max as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > max {
        bail!("Pilot input changed beyond its byte budget");
    }
    Ok(bytes)
}

fn hex_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn load(options: &Options) -> Result<(Vec<JudgmentRun>, Labels)> {
    if options.audit_seed.len() < 16 || options.audit_seed.len() > 256 {
        bail!("Audit seed must be 16..=256 bytes");
    }
    let label_bytes = bounded(&options.labels, 1024 * 1024)?;
    let labels: Labels = serde_json::from_slice(&label_bytes)?;
    if labels.schema_version != 1
        || !hex_digest(&labels.repository_digest)
        || !hex_digest(&labels.question_digest)
        || !hex_digest(&labels.provider_binding_digest)
        || !hex_digest(&labels.model_digest)
        || !hex_digest(&labels.calibrator_digest)
        || !hex_digest(&labels.label_source_digest)
        || labels.target_event != "finding_valid_after_independent_review"
        || labels.validation_from_unix >= labels.validation_until_unix
        || !labels.audit_fraction.is_finite()
        || !(0.0..=1.0).contains(&labels.audit_fraction)
        || labels.labels.len() > MAX_RUNS
    {
        bail!("Pilot labels have invalid scope, target event, period, audit fraction or inventory");
    }
    let mut entries = Vec::new();
    for entry in fs::read_dir(&options.runs)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            bail!("Pilot run inventory contains a non-directory entry");
        }
        entries.push(entry.path());
        if entries.len() > MAX_RUNS {
            bail!("Pilot run inventory exceeds {MAX_RUNS}");
        }
    }
    entries.sort();
    let mut runs = Vec::new();
    let mut total = label_bytes.len();
    for directory in entries {
        let bytes = bounded(&directory.join("decision.json"), 256 * 1024)?;
        total += bytes.len();
        let run: JudgmentRun =
            serde_json::from_slice(&bytes).context("Malformed retained judgment decision")?;
        if directory.file_name().and_then(|name| name.to_str())
            != run.decision_id.strip_prefix("sha256:")
        {
            bail!("Judgment directory does not match decision identity");
        }
        let canonical = fs::canonicalize(&directory)?;
        for artifact in &run.artifacts {
            let path = Path::new(&artifact.path);
            if !fs::canonicalize(path)?.starts_with(&canonical) || artifact.bytes > 16 * 1024 * 1024
            {
                bail!("Judgment artifact escapes its bounded decision directory");
            }
            let source = bounded(path, artifact.bytes as usize)?;
            total += source.len();
            if source.len() as u64 != artifact.bytes || snapshot::digest(&source) != artifact.digest
            {
                bail!("Judgment artifact digest differs from retained evidence");
            }
            if total > MAX_TOTAL_BYTES {
                bail!("Pilot input evidence exceeds 64 MiB");
            }
        }
        runs.push(run);
    }
    Ok((runs, labels))
}

#[derive(Clone)]
struct Observation {
    probability: f64,
    label: bool,
    rule: String,
    language: String,
    threshold: Option<f64>,
}

fn summarize_sync(options: Options) -> Result<(Value, u8)> {
    let (runs, labels) = load(&options)?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let mut issues = Vec::new();
    if runs.is_empty() {
        issues.push("No retained judgment runs".to_owned());
    }
    let mut label_index = BTreeMap::new();
    for label in &labels.labels {
        if !hex_digest(&label.decision_id)
            || !hex_digest(&label.source_digest)
            || label.reviewer.trim().is_empty()
            || !label.independent
            || label.reviewed_unix < labels.validation_from_unix
            || label.reviewed_unix > labels.validation_until_unix
            || label_index
                .insert(label.decision_id.as_str(), label)
                .is_some()
        {
            issues.push(format!(
                "Invalid or duplicate independent label: {}",
                label.decision_id
            ));
        }
    }
    let mut observations = Vec::new();
    let mut high_priority = Vec::new();
    let mut manual_review = 0;
    let mut abstained = 0;
    let mut gaps = 0;
    let mut seen = BTreeSet::new();
    for run in &runs {
        if !seen.insert(&run.decision_id) {
            issues.push(format!("Repeated decision: {}", run.decision_id));
        }
        if run.repository_digest != labels.repository_digest
            || run.question_digest != labels.question_digest
            || run.provider_binding_digest.as_deref() != Some(&labels.provider_binding_digest)
        {
            issues.push(format!(
                "Repository, question or provider binding changed: {}",
                run.decision_id
            ));
        }
        let started = run.audit["started_unix_nanos"]
            .as_u64()
            .map(|value| value / 1_000_000_000)
            .unwrap_or(0);
        if started < labels.validation_from_unix
            || started > labels.validation_until_unix
            || started > now + 60
        {
            issues.push(format!(
                "Judgment is outside the validation period: {}",
                run.decision_id
            ));
        }
        if run.mode == JudgmentMode::Advisory
            && ["manual_review", "prioritize_review"].contains(&run.route.as_str())
        {
            manual_review += 1;
        }
        match &run.assessment {
            Assessment::Probabilistic {
                distribution,
                target_event,
                model_digest,
                ..
            } => {
                let Some(calibration) = run.calibration.as_ref() else {
                    issues.push(format!("Missing calibration: {}", run.decision_id));
                    continue;
                };
                if !run.complete
                    || run.calibration_status != Some(CalibrationStatus::Valid)
                    || model_digest != &labels.model_digest
                    || calibration.model_digest != labels.model_digest
                    || calibration.calibrator_digest != labels.calibrator_digest
                    || calibration.label_source_digest != labels.label_source_digest
                    || calibration.target_event != labels.target_event
                    || target_event != &labels.target_event
                    || calibration.question_digest != labels.question_digest
                    || calibration.repository_digest != labels.repository_digest
                    || calibration.provider_digest != labels.provider_binding_digest
                    || calibration.labeled_until_unix >= labels.validation_from_unix
                    || calibration.expires_unix <= started
                {
                    issues.push(format!(
                        "Calibration or label cohort is invalid: {}",
                        run.decision_id
                    ));
                    continue;
                }
                let Some(probability) = distribution.get("valid") else {
                    issues.push(format!(
                        "Target event choice is absent: {}",
                        run.decision_id
                    ));
                    continue;
                };
                if !probability.is_finite() || !(0.0..=1.0).contains(probability) {
                    issues.push(format!("Invalid probability: {}", run.decision_id));
                    continue;
                }
                if run.route == "prioritize_review" {
                    high_priority.push(run.decision_id.clone());
                }
                if let Some(label) = label_index.get(run.decision_id.as_str()) {
                    if label.reviewed_unix < started {
                        issues.push(format!("Label predates judgment: {}", run.decision_id));
                        continue;
                    }
                    observations.push(Observation {
                        probability: *probability,
                        label: label.finding_valid,
                        rule: run.rule_id.clone(),
                        language: run.language.clone(),
                        threshold: run.review_priority_threshold,
                    });
                }
            }
            Assessment::Abstained { .. } => abstained += 1,
            Assessment::ExecutionGap { .. } => {
                gaps += 1;
                issues.push(format!("Execution gap: {}", run.decision_id));
            }
            Assessment::Deterministic { .. } => {}
        }
    }
    let mut ordered = high_priority.clone();
    ordered.sort_by_key(|id| snapshot::digest(format!("{}:{id}", options.audit_seed).as_bytes()));
    let audit_size = if ordered.is_empty() {
        0
    } else {
        ((ordered.len() as f64 * labels.audit_fraction).ceil() as usize).max(1)
    };
    let sampled: Vec<_> = ordered.into_iter().take(audit_size).collect();
    for id in &sampled {
        if !label_index.contains_key(id.as_str()) {
            issues.push(format!("Audit sample lacks an independent label: {id}"));
        }
    }
    if observations.is_empty() {
        issues.push("No independent labels for calibrated probability outcomes".into());
    }
    let brier = (!observations.is_empty()).then(|| {
        observations
            .iter()
            .map(|item| (item.probability - f64::from(item.label)).powi(2))
            .sum::<f64>()
            / observations.len() as f64
    });
    let log_loss = (!observations.is_empty()).then(|| {
        observations
            .iter()
            .map(|item| {
                let p = item.probability.clamp(1e-12, 1.0 - 1e-12);
                if item.label { -p.ln() } else { -(1.0 - p).ln() }
            })
            .sum::<f64>()
            / observations.len() as f64
    });
    let reliability:Vec<Value>=(0..5).map(|bin| {
        let group:Vec<_>=observations.iter().filter(|item| ((item.probability*5.0) as usize).min(4)==bin).collect();
        let count=group.len();
        json!({"bin":bin,"count":count,"mean_probability":if count==0 {None} else {Some(group.iter().map(|item|item.probability).sum::<f64>()/count as f64)},"observed_rate":if count==0 {None} else {Some(group.iter().filter(|item|item.label).count() as f64/count as f64)}})
    }).collect();
    let mut risk = observations.clone();
    risk.sort_by(|a, b| {
        b.probability
            .max(1.0 - b.probability)
            .total_cmp(&a.probability.max(1.0 - a.probability))
    });
    let risk_coverage:Vec<Value>=[0.25,0.5,0.75,1.0].into_iter().map(|coverage| { let count=((risk.len() as f64*coverage).ceil() as usize).min(risk.len()); let error=risk.iter().take(count).filter(|item|(item.probability>=0.5)!=item.label).count(); json!({"coverage":coverage,"count":count,"empirical_risk":if count==0 {None} else {Some(error as f64/count as f64)}}) }).collect();
    let near: Vec<_> = observations
        .iter()
        .filter(|item| {
            item.threshold
                .is_some_and(|threshold| (item.probability - threshold).abs() <= 0.1)
        })
        .collect();
    let near_threshold_error = if near.is_empty() {
        None
    } else {
        Some(
            near.iter()
                .map(|item| (item.probability - f64::from(item.label)).abs())
                .sum::<f64>()
                / near.len() as f64,
        )
    };
    let mut slices: BTreeMap<(String, String), Vec<&Observation>> = BTreeMap::new();
    for item in &observations {
        slices
            .entry((item.rule.clone(), item.language.clone()))
            .or_default()
            .push(item);
    }
    let slices:Vec<Value>=slices.into_iter().map(|((rule,language),group)| json!({"rule":rule,"language":language,"count":group.len(),"brier":group.iter().map(|item|(item.probability-f64::from(item.label)).powi(2)).sum::<f64>()/group.len() as f64})).collect();
    let complete = issues.is_empty();
    let result = json!({"schema_version":1,"kind":"judgment_pilot","complete":complete,"decision":if complete {"pass"} else {"incomplete"},"target_event":labels.target_event,"repository_digest":labels.repository_digest,"question_digest":labels.question_digest,"provider_binding_digest":labels.provider_binding_digest,"model_digest":labels.model_digest,"calibrator_digest":labels.calibrator_digest,"label_source_digest":labels.label_source_digest,"validation_period":{"from_unix":labels.validation_from_unix,"until_unix":labels.validation_until_unix},"counts":{"runs":runs.len(),"probabilistic_labeled":observations.len(),"independent_labels":labels.labels.len(),"manual_review_recommendations":manual_review,"abstained":abstained,"execution_gaps":gaps,"high_priority":high_priority.len()},"calibration":{"brier_score":brier,"log_loss":log_loss,"near_threshold_error":near_threshold_error,"reliability":reliability,"risk_coverage":risk_coverage,"slices":slices},"audit":{"seed_digest":snapshot::digest(options.audit_seed.as_bytes()),"fraction":labels.audit_fraction,"sampled_decision_ids":sampled,"missing_labels":issues.iter().filter(|issue|issue.starts_with("Audit sample lacks")).count()},"issues":issues,"gate_effect":"none"});
    Ok((result, if complete { 0 } else { 2 }))
}

pub async fn summarize(options: Options) -> Result<(Value, u8)> {
    tokio::task::spawn_blocking(move || summarize_sync(options)).await?
}
