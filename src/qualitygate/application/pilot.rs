//! Filesystem loading and CPU work are isolated from asynchronous orchestration.
use super::external;
use anyhow::{Context, Result};
use serde_json::Value;
use std::path::{Path, PathBuf};

pub async fn seal(input: PathBuf) -> Result<(Value, u8)> {
    tokio::task::spawn_blocking(move || {
        let (manifest, reports) = crate::config::pilot::load(&input)?;
        crate::domain::pilot::verify_initial_baselines(&manifest, &reports)?;
        let now = crate::config::policy_store::now()?;
        let sealed = crate::domain::pilot::seal(manifest, now)?;
        crate::config::pilot::verify_sources(&input, &sealed)?;
        crate::config::pilot::verify_initial_reports(&input, &sealed)?;
        crate::config::pilot::verify_model_evidence(&input, &sealed)?;
        crate::config::pilot::verify_execution_evidence(&input, &sealed)?;
        Ok((serde_json::to_value(sealed)?, 0))
    })
    .await?
}

pub async fn authorization_subject(input: PathBuf) -> Result<(Value, u8)> {
    tokio::task::spawn_blocking(move || {
        let (manifest, reports) = crate::config::pilot::load(&input)?;
        crate::domain::pilot::verify_initial_baselines(&manifest, &reports)?;
        if !manifest.observations.is_empty() {
            anyhow::bail!("Pilot start authorization subject requires no observations");
        }
        let subject = crate::domain::pilot::authorization_subject(&manifest)?;
        crate::config::pilot::verify_sources(&input, &manifest)?;
        crate::config::pilot::verify_initial_reports(&input, &manifest)?;
        crate::config::pilot::verify_model_evidence(&input, &manifest)?;
        crate::config::pilot::verify_execution_evidence(&input, &manifest)?;
        Ok((
            serde_json::json!({
                "schema_version": 1,
                "payload_type": crate::adapters::pilot_authorization::PAYLOAD_TYPE,
                "subject": subject,
            }),
            0,
        ))
    })
    .await?
}

struct PlanAuthentication {
    verified: crate::domain::pilot::VerifiedPlanAuthorization,
    provenance: Value,
    inputs: external::SingleInput,
}

struct TrialAuthentication {
    verified: crate::domain::pilot::VerifiedPilotAcceptance,
    provenance: Value,
    inputs: external::SingleInput,
}

fn provenance(inputs: &external::SingleInput, label: &str) -> Result<Value> {
    let trust_source = inputs
        .store_path
        .to_str()
        .context("Trust store path must be valid UTF-8 for evidence output")?;
    let record_source = inputs
        .record_path
        .to_str()
        .context("Signed record path must be valid UTF-8 for evidence output")?;
    let mut value = serde_json::json!({
        "trust_source": trust_source,
        "trust_digest": crate::snapshot::digest(&inputs.store_bytes),
    });
    let object = value.as_object_mut().expect("provenance object");
    object.insert(format!("{label}_source"), serde_json::json!(record_source));
    object.insert(
        format!("{label}_digest"),
        serde_json::json!(crate::snapshot::digest(&inputs.record_bytes)),
    );
    Ok(value)
}

fn authenticate_plan(
    root: &Path,
    trust_store: &Path,
    authorization: &Path,
    manifest: &crate::domain::pilot::Manifest,
    now: u64,
) -> Result<PlanAuthentication> {
    let inputs = external::load_single(
        root,
        trust_store,
        authorization,
        crate::adapters::pilot_authorization::MAX_ENVELOPE_BYTES,
    )?;
    let subject = crate::domain::pilot::authorization_subject(manifest)?;
    let verified = crate::adapters::pilot_authorization::verify(
        &inputs.record_bytes,
        &inputs.store,
        &subject,
        now,
    )?;
    Ok(PlanAuthentication {
        verified,
        provenance: provenance(&inputs, "authorization")?,
        inputs,
    })
}

fn authenticate_acceptance(
    root: &Path,
    trust_store: &Path,
    acceptance: &Path,
    expected: &crate::domain::pilot::PilotAcceptanceSubject,
    now: u64,
) -> Result<TrialAuthentication> {
    let inputs = external::load_single(
        root,
        trust_store,
        acceptance,
        crate::adapters::pilot_acceptance::MAX_ENVELOPE_BYTES,
    )?;
    let verified = crate::adapters::pilot_acceptance::verify(
        &inputs.record_bytes,
        &inputs.store,
        expected,
        now,
    )?;
    Ok(TrialAuthentication {
        verified,
        provenance: provenance(&inputs, "acceptance")?,
        inputs,
    })
}

pub async fn acceptance_subject(
    root: PathBuf,
    input: PathBuf,
    trust_store: PathBuf,
    authorization: PathBuf,
) -> Result<(Value, u8)> {
    tokio::task::spawn_blocking(move || {
        let (manifest, reports) = crate::config::pilot::load(&input)?;
        let now = crate::config::policy_store::now()?;
        let plan = authenticate_plan(&root, &trust_store, &authorization, &manifest, now)?;
        let subject =
            crate::domain::pilot::acceptance_subject(&manifest, &reports, now, &plan.verified)?;
        external::single_unchanged(&plan.inputs)?;
        crate::config::pilot::verify_sources(&input, &manifest)?;
        crate::config::pilot::verify_initial_reports(&input, &manifest)?;
        crate::config::pilot::verify_model_evidence(&input, &manifest)?;
        crate::config::pilot::verify_execution_evidence(&input, &manifest)?;
        Ok((
            serde_json::json!({
                "schema_version":1,
                "payload_type":crate::adapters::pilot_acceptance::PAYLOAD_TYPE,
                "subject":subject,
            }),
            0,
        ))
    })
    .await?
}

pub async fn summarize(
    root: PathBuf,
    input: PathBuf,
    trust_store: Option<PathBuf>,
    authorization: Option<PathBuf>,
    acceptance: Option<PathBuf>,
) -> Result<(Value, u8)> {
    tokio::task::spawn_blocking(move || {
        let (manifest, reports) = crate::config::pilot::load(&input)?;
        let now = crate::config::policy_store::now()?;
        let verified = match (trust_store, authorization) {
            (Some(store), Some(record)) => Some((
                authenticate_plan(&root, &store, &record, &manifest, now)?,
                store,
            )),
            (None, None) => None,
            _ => anyhow::bail!("Pilot authorization requires both trust store and signed record"),
        };
        let trial = match (acceptance, verified.as_ref()) {
            (Some(record), Some((plan, store))) => {
                let expected = crate::domain::pilot::acceptance_subject(
                    &manifest,
                    &reports,
                    now,
                    &plan.verified,
                )?;
                Some(authenticate_acceptance(
                    &root, store, &record, &expected, now,
                )?)
            }
            (None, _) => None,
            (Some(_), None) => anyhow::bail!(
                "Pilot acceptance requires trust store and authenticated start authorization"
            ),
        };
        let mut summary = crate::domain::pilot::summarize_with_acceptance(
            &manifest,
            &reports,
            now,
            verified.as_ref().map(|(plan, _)| &plan.verified),
            trial.as_ref().map(|value| &value.verified),
        )?;
        if let Some((plan, _)) = &verified {
            let evidence = summary["plan_authorization"]
                .as_object_mut()
                .context("Pilot authorization summary must be an object")?;
            evidence.extend(
                plan.provenance
                    .as_object()
                    .context("Pilot authorization provenance must be an object")?
                    .clone(),
            );
            external::single_unchanged(&plan.inputs)?;
        }
        if let Some(trial) = &trial {
            let evidence = summary["trial_acceptance_evidence"]
                .as_object_mut()
                .context("Pilot acceptance summary must be an object")?;
            evidence.extend(
                trial
                    .provenance
                    .as_object()
                    .context("Pilot acceptance provenance must be an object")?
                    .clone(),
            );
            external::single_unchanged(&trial.inputs)?;
        }
        crate::config::pilot::verify_sources(&input, &manifest)?;
        crate::config::pilot::verify_initial_reports(&input, &manifest)?;
        crate::config::pilot::verify_model_evidence(&input, &manifest)?;
        crate::config::pilot::verify_execution_evidence(&input, &manifest)?;
        let code = if summary["complete"] != true {
            2
        } else if summary["trial_acceptance"] == "rejected" {
            1
        } else {
            0
        };
        Ok((summary, code))
    })
    .await?
}
