//! Authenticate the selected policy against live external trust, including revocations.

use crate::{
    adapters::policy_approval,
    config::{
        self,
        policy_acceptance::{EvolutionTrust, ProtectedFile},
        policy_active::{Active, Authorization},
        policy_store::Store,
    },
    domain::{ManualDecision, PolicyEvidence},
    snapshot::Snapshot,
};
use anyhow::{Result, bail};
use std::path::{Path, PathBuf};

pub struct Authenticated {
    pub active: Active,
    trust: ProtectedFile,
}

fn authenticate(root: &Path) -> Result<Option<Authenticated>> {
    let Some(active) = config::policy_active::load(root)? else {
        return Ok(None);
    };
    if let Authorization::Rollback(approved) = &active.authority {
        let file = ProtectedFile::read(root, approved.trust_source.as_ref())?;
        let trust: EvolutionTrust = serde_norway::from_slice(&file.bytes)?;
        if trust.repository != dunce::canonicalize(root)?.to_string_lossy() {
            bail!("Rollback belongs to another repository");
        }
        let verified = crate::adapters::policy_rollback::verify(
            &Store::open(root)?.blob(&approved.envelope_ref)?,
            &trust,
            &approved.approval.subject,
            active.since,
        )?;
        if verified.signer_key_id != approved.signer_key_id
            || verified.public_key_digest != approved.public_key_digest
        {
            bail!("Active rollback signer identity differs from its authorization");
        }
        file.unchanged(root)?;
        return Ok(Some(Authenticated {
            active,
            trust: file,
        }));
    }
    let Authorization::Promotion(approved) = &active.authority else {
        unreachable!("rollback handled above")
    };
    let file = ProtectedFile::read(root, approved.trust_source.as_ref())?;
    let trust: EvolutionTrust = serde_norway::from_slice(&file.bytes)?;
    config::policy_acceptance::validate_trust(&trust)?;
    let subject = &approved.approval.subject;
    if trust.repository != dunce::canonicalize(root)?.to_string_lossy()
        || trust.repository != subject.repository
        || !trust.suites.contains(&subject.suite_digest)
        || !trust.baselines.contains(&subject.parent_policy)
        || !trust.evaluators.contains(&subject.evaluator_digest)
    {
        bail!("Active policy is no longer authorized by live external trust");
    }
    let verified = policy_approval::verify(
        &Store::open(root)?.blob(&approved.envelope_ref)?,
        &trust,
        subject,
        active.since,
    )?;
    if verified.approval.decision != ManualDecision::Approved
        || verified.signer_key_id != approved.signer_key_id
        || verified.public_key_digest != approved.public_key_digest
    {
        bail!("Active policy approval identity is invalid");
    }
    file.unchanged(root)?;
    Ok(Some(Authenticated {
        active,
        trust: file,
    }))
}

pub async fn load(root: PathBuf) -> Result<Option<Authenticated>> {
    tokio::task::spawn_blocking(move || authenticate(&root)).await?
}

pub async fn revalidate(root: PathBuf, initial: Authenticated) -> Result<()> {
    tokio::task::spawn_blocking(move || {
        initial.trust.unchanged(&root)?;
        let current = authenticate(&root)?
            .ok_or_else(|| anyhow::anyhow!("Active policy disappeared during execution"))?;
        if current.active.reference != initial.active.reference
            || current.active.approval_ref != initial.active.approval_ref
            || current.active.since != initial.active.since
        {
            bail!("Active policy changed during execution");
        }
        Ok(())
    })
    .await?
}

pub(super) fn prepare(
    active: &Active,
    snapshot: &Snapshot,
    options: &super::CheckOptions,
    invalid: &mut Vec<String>,
) -> Result<super::policy::Loaded> {
    let (config, catalog) = active.frozen.resolve(&active.config)?;
    if options
        .policy_ref
        .as_ref()
        .is_some_and(|reference| reference != &active.reference)
    {
        bail!(
            "An active signed policy is selected; historical policy replay uses candidate validate"
        );
    }
    let store = Store::open(&options.root)?;
    let task_bytes = options
        .task
        .as_ref()
        .map(|path| {
            let asset = active.version.files.get(path).ok_or_else(|| {
                anyhow::anyhow!(
                    "Task must be retained in the active policy's verification_assets: {path}"
                )
            })?;
            store.blob(&asset.digest)
        })
        .transpose()?;
    let task = task_bytes.as_deref().map(config::parse_task).transpose()?;
    let mut changes = Vec::new();
    for (path, asset) in &active.version.files {
        if path == &active.version.config_path {
            continue;
        }
        if !snapshot.files.get(path).is_some_and(|file| {
            crate::snapshot::digest(&file.bytes) == asset.digest
                && file.executable == asset.executable
        }) {
            changes.push(path.clone());
            invalid.push(format!(
                "Active policy verification asset differs from the checked snapshot: {path}"
            ));
        }
    }
    let evidence = PolicyEvidence {
        source: active.reference.clone(),
        resolved_commit: Some(active.version.source_commit.clone()),
        config_digest: active.version.files[&active.version.config_path]
            .digest
            .clone(),
        rules_digest: config::policy_promotion::rules_digest(&config, &catalog)?,
        task_contract_digest: task_bytes.as_deref().map(crate::snapshot::digest),
        task_contract_source: options.task.clone(),
        trust: "signed_active_policy".into(),
        changes,
        source_reviews: config::source_reviews::evidence(&config, &catalog)?,
    };
    Ok(super::policy::Loaded {
        protected_paths: super::policy::protected_paths(
            &config,
            &catalog,
            &active.version.config_path,
            options.task.as_deref(),
        ),
        evidence,
        catalog,
        plan: config::Plan::build(&config, task.as_ref(), &options.profile)?,
    })
}

pub async fn configuration(root: PathBuf, path: String) -> Result<serde_json::Value> {
    let active = load(root.clone()).await?;
    tokio::task::spawn_blocking(move || {
        let (config, catalog, source, trust) = if let Some(active) = active {
            let (config, catalog) = active.active.frozen.resolve(&active.active.config)?;
            (config, catalog, active.active.reference, "signed_active_policy")
        } else {
            let config = config::read(&root, path.as_ref())?;
            let catalog = config::catalog::read(&root, &config)?;
            (catalog.resolve(&config)?, catalog, path, "local_candidate")
        };
        Ok(serde_json::json!({"schema_version":1,"source":source,"config":config,"catalog":catalog,"review_trust":trust}))
    }).await?
}
