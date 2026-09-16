//! Immutable policy packages and evidence-linked candidate revisions.

use super::{
    Config,
    catalog::{Catalog, Entry},
    policy_store::{Store, now},
    rule_management::Mutation,
    rule_query::Inventory,
};
use crate::domain::evolution::*;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrozenCatalog {
    pub builtins: Catalog,
    pub projects: BTreeMap<String, Entry>,
}

impl FrozenCatalog {
    pub fn resolve(&self, config: &Config) -> Result<(Config, Catalog)> {
        let inventory = Inventory::from_catalogs(
            Some(config.clone()),
            self.builtins.clone(),
            self.projects.clone(),
            config
                .custom_rules
                .clone()
                .unwrap_or_else(|| super::catalog::PROJECT_RULES_DIR.into()),
        )?;
        let selected = inventory.selected(config);
        Ok((selected.resolve(config)?, selected))
    }
}

/// Arguments shared by creation from a Git snapshot and from an archived version.
pub struct Proposal {
    pub actor: Actor,
    pub reason: String,
    pub evidence: Vec<String>,
}

pub fn retain_from_files(root: &Path, input: &str, source: &str) -> Result<Value> {
    let record: EvidenceRecord =
        serde_json::from_slice(&super::rule_authoring::read_file(root, input)?)?;
    let source = super::rule_authoring::read_file(root, source)?;
    retain_evidence(root, record, &source)
}

pub fn show(root: &Path, kind: &str, id: &str) -> Result<Value> {
    let store = Store::open(root)?;
    match kind {
        "evidence" => Ok(
            json!({"schema_version":1,"evidence_ref":id,"record":evidence(&store, id)?,"trust":"unverified_evidence"}),
        ),
        "candidate" => {
            let (reference, revision) = candidate(&store, id)?;
            let (_, config, _) = load_version(&store, &revision.policy_digest)?;
            Ok(
                json!({"schema_version":1,"id":id,"revision_ref":reference,"revision":revision,"config":config}),
            )
        }
        "policy" => {
            let (version, config, catalog) = load_version(&store, id)?;
            Ok(
                json!({"schema_version":1,"policy_digest":id,"version":version,"config":config,"catalog":catalog,"active":store.index.active_policy.as_deref() == Some(id)}),
            )
        }
        _ => bail!("Unknown policy archive query kind"),
    }
}

pub fn list(root: &Path, kind: &str, offset: usize, limit: usize) -> Result<Value> {
    if !(1..=256).contains(&limit) {
        bail!("Archive query limit must be 1..256");
    }
    let store = Store::open(root)?;
    let (rows, count): (Vec<Value>, usize) = match kind {
        "evidence" => (store.index.evidence.iter().skip(offset).take(limit).map(|reference| json!({"evidence_ref":reference})).collect(), store.index.evidence.len()),
        "candidate" => (store.index.candidates.iter().skip(offset).take(limit).map(|(id, reference)| {
            let revision: PolicyRevision = store.record(reference, "candidate")?;
            Ok(json!({"id":id,"revision_ref":reference,"status":revision.status,"parent_policy_digest":revision.parent_policy_digest,"policy_digest":revision.policy_digest,"created_by":revision.created_by,"reason":revision.reason}))
        }).collect::<Result<_>>()?, store.index.candidates.len()),
        _ => bail!("Unknown policy archive listing kind"),
    };
    let next = offset.saturating_add(rows.len());
    Ok(
        json!({"schema_version":1,"kind":kind,"records":rows,"total":count,"next_offset":if next < count { Some(next) } else { None }}),
    )
}

fn audit(
    store: &mut Store,
    action: &str,
    subject: &str,
    actor: &Actor,
    reason: &str,
) -> Result<()> {
    store.event(PolicyTransition {
        sequence: 0,
        previous: None,
        action: action.into(),
        subject: subject.into(),
        actor: actor.clone(),
        reason: reason.into(),
        timestamp: now()?,
        from_policy: store.index.active_policy.clone(),
        to_policy: store.index.active_policy.clone(),
    })
}

pub fn retain_evidence(root: &Path, record: EvidenceRecord, source: &[u8]) -> Result<Value> {
    record.validate().map_err(anyhow::Error::msg)?;
    if super::policy_store::digest(source) != record.source_digest {
        bail!("Evidence source digest does not match the retained bytes");
    }
    Store::transaction(root, |store| {
        super::case_provenance::validate_record(store, &record)?;
        store.put_blob(source)?;
        let reference = store.put_record("evidence", &record)?;
        if !store.index.evidence.contains(&reference) {
            store.index.evidence.push(reference.clone());
            audit(
                store,
                "evidence_retained",
                &reference,
                &record.actor,
                "Source bytes and unverified claims retained",
            )?;
        }
        Ok(
            json!({"schema_version":1,"evidence_ref":reference,"record":record,"trust":"unverified_evidence"}),
        )
    })
}

pub fn evidence(store: &Store, reference: &str) -> Result<EvidenceRecord> {
    if !store.index.evidence.iter().any(|id| id == reference) {
        bail!("Evidence is not published in this archive");
    }
    let record: EvidenceRecord = store.record(reference, "evidence")?;
    record.validate().map_err(anyhow::Error::msg)?;
    store.blob(&record.source_digest)?;
    Ok(record)
}

pub fn create_from_version(root: &Path, parent: &str, proposal: Proposal) -> Result<Value> {
    Store::transaction(root, |store| create(store, parent, &proposal))
}

/// Iterates borrowed snapshot inputs once; source-tree blobs are neither cloned nor archived.
pub fn create_from_snapshot<'a>(
    root: &Path,
    config_path: &str,
    commit: &str,
    files: impl IntoIterator<Item = (&'a str, &'a [u8], bool)>,
    proposal: Proposal,
) -> Result<Value> {
    let files: BTreeMap<_, _> = files
        .into_iter()
        .map(|(name, bytes, executable)| (name, (bytes, executable)))
        .collect();
    let bytes = files
        .get(config_path)
        .context("Parent snapshot is missing the policy configuration")?
        .0;
    let config = super::parse(bytes)?;
    let builtins = Catalog::load(
        &Config {
            rulesets: super::RULESETS.iter().map(|name| (*name).into()).collect(),
            ..Config::default()
        },
        std::iter::empty(),
    )?;
    let directory = config
        .custom_rules
        .as_deref()
        .unwrap_or(super::catalog::PROJECT_RULES_DIR);
    let project_config = Config {
        custom_rules: Some(directory.into()),
        ..Config::default()
    };
    let has_projects = files.keys().any(|name| {
        name.starts_with(&format!("{directory}/"))
            && (name.ends_with(".yaml") || name.ends_with(".yml"))
    });
    let projects = if has_projects || config.custom_rules.is_some() {
        super::project_inventory::parse(
            &project_config,
            files.iter().map(|(name, (bytes, _))| (*name, *bytes)),
            super::parallel::jobs(),
        )?
    } else {
        BTreeMap::new()
    };
    let frozen = FrozenCatalog { builtins, projects };
    let inventory = Inventory::from_catalogs(
        Some(config.clone()),
        frozen.builtins.clone(),
        frozen.projects.clone(),
        directory.into(),
    )?;
    inventory.selected(&config).resolve(&config)?;
    let mut names = BTreeSet::from([config_path.to_owned()]);
    let mut builder = globset::GlobSetBuilder::new();
    for pattern in &config.verification_assets {
        builder.add(globset::Glob::new(pattern)?);
    }
    builder.add(globset::Glob::new(&format!("{directory}/**"))?);
    let matcher = builder.build()?;
    names.extend(
        files
            .keys()
            .filter(|name| matcher.is_match(name))
            .map(|name| (*name).to_owned()),
    );
    names.extend(
        config
            .rules
            .values()
            .filter_map(|rule| rule.source.as_ref())
            .map(|source| source.document.clone()),
    );
    names.extend(
        frozen
            .projects
            .values()
            .filter_map(|entry| entry.custom.as_ref())
            .map(|rule| rule.source.document.clone()),
    );
    if names.len() > 4096 {
        bail!("Policy package exceeds 4096 files");
    }
    let total: usize = names
        .iter()
        .map(|name| files.get(name.as_str()).map_or(0, |(bytes, _)| bytes.len()))
        .sum();
    if total > 32 * 1024 * 1024 {
        bail!("Policy package exceeds 32 MiB");
    }
    Store::transaction(root, |store| {
        let mut published = BTreeMap::new();
        for name in names {
            if crate::paths::relative(Path::new(&name))? != name {
                bail!("Non-normalized policy package path");
            }
            let (bytes, executable) = files
                .get(name.as_str())
                .with_context(|| format!("Policy input is missing: {name}"))?;
            published.insert(
                name,
                PolicyFile {
                    digest: store.put_blob(bytes)?,
                    executable: *executable,
                },
            );
        }
        let catalog_digest = store.put_record("catalog", &frozen)?;
        let version = PolicyVersion {
            schema_version: 1,
            config_path: config_path.into(),
            files: published,
            catalog_digest,
            source_commit: commit.into(),
        };
        let parent = store.put_record("policy", &version)?;
        if !store.index.policies.contains(&parent) {
            store.index.policies.push(parent.clone());
        }
        create(store, &parent, &proposal)
    })
}

fn create(store: &mut Store, parent: &str, proposal: &Proposal) -> Result<Value> {
    load_version(store, parent)?;
    let revision = PolicyRevision {
        schema_version: 1,
        parent_policy_digest: parent.into(),
        previous_revision: None,
        policy_digest: parent.into(),
        patch: Vec::new(),
        status: RevisionStatus::Candidate,
        created_by: proposal.actor.clone(),
        reason: proposal.reason.clone(),
        evidence_refs: proposal.evidence.clone(),
        created_at: now()?,
        evaluation_ref: None,
        approval_ref: None,
    };
    revision.validate().map_err(anyhow::Error::msg)?;
    for reference in &revision.evidence_refs {
        evidence(store, reference)?;
    }
    let reference = store.put_record("candidate", &revision)?;
    let id = format!("candidate-{:08}", store.index.sequence + 1);
    if store.index.candidates.contains_key(&id) {
        bail!("Candidate ID already exists");
    }
    store.index.candidates.insert(id.clone(), reference.clone());
    audit(
        store,
        "candidate_created",
        &reference,
        &proposal.actor,
        &proposal.reason,
    )?;
    Ok(
        json!({"schema_version":1,"id":id,"revision_ref":reference,"revision":revision,"trust":"unapproved_candidate"}),
    )
}

pub fn load_version(
    store: &Store,
    reference: &str,
) -> Result<(PolicyVersion, Config, FrozenCatalog)> {
    if !store.index.policies.iter().any(|id| id == reference) {
        bail!("Policy version is not published in this archive");
    }
    let version: PolicyVersion = store.record(reference, "policy")?;
    if version.schema_version != 1 || version.files.len() > 4096 || version.source_commit.is_empty()
    {
        bail!("Invalid policy version schema or input inventory");
    }
    let file = version
        .files
        .get(&version.config_path)
        .context("Policy version is missing its configuration")?;
    let config: Config = super::parse_yaml(&store.blob(&file.digest)?)?;
    super::validation::layout(&config, false)?;
    let catalog: FrozenCatalog = store.record(&version.catalog_digest, "catalog")?;
    let mut total = 0;
    for (path, file) in &version.files {
        if crate::paths::relative(Path::new(path))? != *path {
            bail!("Invalid policy version path");
        }
        total += store.blob(&file.digest)?.len();
        if total > 32 * 1024 * 1024 {
            bail!("Policy package exceeds 32 MiB");
        }
    }
    Inventory::from_catalogs(
        Some(config.clone()),
        catalog.builtins.clone(),
        catalog.projects.clone(),
        config
            .custom_rules
            .clone()
            .unwrap_or_else(|| super::catalog::PROJECT_RULES_DIR.into()),
    )?;
    Ok((version, config, catalog))
}

pub fn candidate(store: &Store, id: &str) -> Result<(String, PolicyRevision)> {
    let reference = store
        .index
        .candidates
        .get(id)
        .context("Unknown candidate ID")?;
    let revision: PolicyRevision = store.record(reference, "candidate")?;
    revision.validate().map_err(anyhow::Error::msg)?;
    Ok((reference.clone(), revision))
}

pub fn mutate(root: &Path, id: &str, actor: Actor, mutation: Mutation) -> Result<Value> {
    actor.validate().map_err(anyhow::Error::msg)?;
    Store::transaction(root, |store| {
        let (previous, mut revision) = candidate(store, id)?;
        if !revision.editable() {
            bail!(
                "Candidate is frozen in {:?}; create a new candidate for further edits",
                revision.status
            );
        }
        if let super::rule_management::Mutation::Lifecycle { record, .. } = &mutation
            && (record.actor != actor || record.evidence_refs != revision.evidence_refs)
        {
            bail!("Lifecycle changes must bind the candidate's evidence and editing actor");
        }
        let (mut version, config, catalog) = load_version(store, &revision.policy_digest)?;
        let original = store.blob(&version.files[&version.config_path].digest)?;
        let mut value = serde_json::to_value(super::parse_yaml::<serde_norway::Value>(&original)?)?;
        let inventory = Inventory::from_catalogs(
            Some(config.clone()),
            catalog.builtins,
            catalog.projects,
            config
                .custom_rules
                .clone()
                .unwrap_or_else(|| super::catalog::PROJECT_RULES_DIR.into()),
        )?;
        let result =
            super::rule_management::edit_value(&mut value, &config, &inventory, &mutation)?;
        let candidate: Config = serde_json::from_value(value.clone())?;
        super::validation::layout(&candidate, false)?;
        inventory.selected(&candidate).resolve(&candidate)?;
        let bytes = serde_norway::to_string(&value)?.into_bytes();
        if bytes.len() > super::MAX_CONFIG_BYTES {
            bail!("Candidate configuration exceeds 1 MiB");
        }
        let changed =
            serde_json::to_value(super::parse_yaml::<serde_norway::Value>(&original)?)? != value;
        if !changed {
            return Ok(
                json!({"schema_version":1,"id":id,"changed":false,"revision_ref":previous,"revision":revision}),
            );
        }
        version
            .files
            .get_mut(&version.config_path)
            .expect("validated config file")
            .digest = store.put_blob(&bytes)?;
        let policy = store.put_record("policy", &version)?;
        if !store.index.policies.contains(&policy) {
            store.index.policies.push(policy.clone());
        }
        revision.policy_digest = policy;
        revision.previous_revision = Some(previous);
        revision
            .patch
            .push(json!({"actor":actor,"mutation":mutation,"timestamp":now()?}));
        revision.evaluation_ref = None;
        revision.approval_ref = None;
        revision.validate().map_err(anyhow::Error::msg)?;
        let reference = store.put_record("candidate", &revision)?;
        store.index.candidates.insert(id.into(), reference.clone());
        audit(
            store,
            "candidate_edited",
            &reference,
            &actor,
            &revision.reason,
        )?;
        Ok(
            json!({"schema_version":1,"id":id,"changed":true,"revision_ref":reference,"revision":revision,"result":result,"trust":"unapproved_candidate"}),
        )
    })
}

pub fn reject(root: &Path, id: &str, actor: Actor, reason: &str) -> Result<Value> {
    actor.validate().map_err(anyhow::Error::msg)?;
    validate_text(reason, 4096).map_err(anyhow::Error::msg)?;
    Store::transaction(root, |store| {
        let (previous, mut revision) = candidate(store, id)?;
        if !revision.editable() {
            bail!(
                "Only a draft or candidate can be rejected without a protected lifecycle transition"
            );
        }
        revision.previous_revision = Some(previous);
        revision.status = RevisionStatus::Rejected;
        let reference = store.put_record("candidate", &revision)?;
        store.index.candidates.insert(id.into(), reference.clone());
        audit(store, "candidate_rejected", &reference, &actor, reason)?;
        Ok(
            json!({"schema_version":1,"id":id,"revision_ref":reference,"revision":revision,"reason":reason}),
        )
    })
}

pub fn history(root: &Path, cursor: Option<&str>, limit: usize) -> Result<Value> {
    if !(1..=256).contains(&limit) {
        bail!("History limit must be 1..256");
    }
    let store = Store::open(root)?;
    let mut next = cursor.map(str::to_owned).or(store.index.last_event.clone());
    let mut events = Vec::new();
    let mut seen = BTreeSet::new();
    while let Some(reference) = &next {
        if events.len() >= limit {
            break;
        }
        if !seen.insert(reference.clone()) {
            bail!("Policy history cycle");
        }
        let event: PolicyTransition = store.record(reference, "transition")?;
        events.push(json!({"event_ref":reference,"event":event}));
        next = event.previous;
    }
    Ok(
        json!({"schema_version":1,"active_policy":store.index.active_policy,"events":events,"next_cursor":next}),
    )
}
