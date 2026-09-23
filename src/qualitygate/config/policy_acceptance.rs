//! Caller-owned validation suites and trust roots, frozen before candidate execution.

use crate::{
    domain::{
        evolution::{Actor, ActorKind, valid_digest},
        policy_evaluation::{CaseKind, EvaluationBudget, Expected},
    },
    paths,
};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
    path::{Path, PathBuf},
    time::SystemTime,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationCase {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_ref: Option<String>,
    pub id: String,
    pub kind: CaseKind,
    pub base: String,
    pub head: String,
    pub task: super::TaskContract,
    pub expectations: BTreeMap<String, Expected>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationSuite {
    pub schema_version: u32,
    pub id: String,
    pub baseline_policy: String,
    pub evaluator_epoch: String,
    pub motivating_evidence: Vec<String>,
    pub budget: EvaluationBudget,
    pub min_improvements: usize,
    pub max_rules: usize,
    pub max_rule_growth: usize,
    pub cases: Vec<ValidationCase>,
}

/// Parses and validates a bounded suite without granting trust or approval.
pub fn parse_suite(bytes: &[u8]) -> Result<ValidationSuite> {
    if bytes.len() > super::MAX_CONFIG_BYTES {
        bail!("Validation suite exceeds {} bytes", super::MAX_CONFIG_BYTES);
    }
    let suite = super::parse_yaml(bytes)?;
    validate_suite(&suite)?;
    Ok(suite)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalKey {
    pub id: String,
    pub actor: Actor,
    pub public_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvolutionTrust {
    pub schema_version: u32,
    pub repository: String,
    pub suites: Vec<String>,
    pub baselines: Vec<String>,
    pub evaluators: Vec<String>,
    pub approval_keys: Vec<ApprovalKey>,
    pub revoked_approvals: Vec<String>,
    pub max_age_seconds: u64,
}

#[derive(Debug, PartialEq, Eq)]
struct Stamp {
    modified: Option<SystemTime>,
    created: Option<SystemTime>,
    readonly: bool,
    #[cfg(unix)]
    native: (u64, u64, i64, i64),
}

impl Stamp {
    fn from(metadata: &std::fs::Metadata) -> Self {
        Self {
            modified: metadata.modified().ok(),
            created: metadata.created().ok(),
            readonly: metadata.permissions().readonly(),
            #[cfg(unix)]
            native: {
                use std::os::unix::fs::MetadataExt;
                (
                    metadata.dev(),
                    metadata.ino(),
                    metadata.ctime(),
                    metadata.ctime_nsec(),
                )
            },
        }
    }
}

pub struct ProtectedFile {
    pub path: PathBuf,
    pub bytes: Vec<u8>,
    stamp: Stamp,
}

impl ProtectedFile {
    pub fn read(root: &Path, path: &Path) -> Result<Self> {
        Self::read_limited(root, path, super::MAX_CONFIG_BYTES)
    }

    pub(crate) fn read_limited(root: &Path, path: &Path, limit: usize) -> Result<Self> {
        Self::read_checked(root, path, limit).map_err(|error| crate::domain::prerequisites::PrerequisiteIssue::new(
            crate::domain::prerequisites::FailureCode::EvidenceInvalid,
            crate::domain::prerequisites::Phase::Evidence, "Protected acceptance input is unavailable or invalid")
            .resource(path.display().to_string()).instruction("Provide the required bounded, readable trust/evidence file outside the checked repository.").wrap(error))
    }

    fn read_checked(root: &Path, path: &Path, limit: usize) -> Result<Self> {
        let root = dunce::canonicalize(root)?;
        let path = if path.is_absolute() {
            path.to_owned()
        } else {
            root.join(path)
        };
        let canonical = dunce::canonicalize(&path)?;
        if canonical.starts_with(&root) {
            bail!("Protected acceptance inputs must be outside the checked repository");
        }
        // Traverse the original absolute path so symlink ancestors cannot redirect trust.
        let anchor = path
            .ancestors()
            .last()
            .context("Protected input has no filesystem root")?;
        let relative = paths::from_native(path.strip_prefix(anchor)?)?;
        if paths::confined(anchor, Path::new(&relative))? != path {
            bail!("Protected path is not confined");
        }
        let metadata = std::fs::symlink_metadata(&path)?;
        if !metadata.is_file() || metadata.len() > limit as u64 {
            bail!(
                "Protected input must be a regular file of at most {} MiB",
                limit / (1024 * 1024)
            );
        }
        let stamp = Stamp::from(&metadata);
        let file = std::fs::File::open(&path)?;
        if !file.metadata()?.is_file() {
            bail!("Protected input changed type");
        }
        let mut bytes = Vec::new();
        file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
        if bytes.len() > limit
            || bytes.len() as u64 != metadata.len()
            || Stamp::from(&std::fs::metadata(&path)?) != stamp
        {
            bail!("Protected input changed during bounded read");
        }
        Ok(Self { path, bytes, stamp })
    }

    pub fn unchanged(&self, root: &Path) -> Result<()> {
        let current = Self::read(root, &self.path)?;
        if current.bytes != self.bytes || current.stamp != self.stamp {
            bail!(
                "Protected acceptance input changed after validation began: {}",
                self.path.display()
            );
        }
        Ok(())
    }
}

pub struct ProtectedInputs {
    pub suite: ValidationSuite,
    pub trust: EvolutionTrust,
    pub suite_file: ProtectedFile,
    pub trust_file: ProtectedFile,
}

impl ProtectedInputs {
    pub fn load(root: &Path, suite_path: &Path, trust_path: &Path) -> Result<Self> {
        let suite_file = ProtectedFile::read(root, suite_path)?;
        let trust_file = ProtectedFile::read(root, trust_path)?;
        let suite = parse_suite(&suite_file.bytes)?;
        let trust: EvolutionTrust = super::parse_yaml(&trust_file.bytes)?;
        validate_trust(&trust)?;
        if Path::new(&trust.repository) != dunce::canonicalize(root)? {
            bail!("Trust root belongs to a different repository");
        }
        if !trust
            .suites
            .contains(&super::policy_store::digest(&suite_file.bytes))
            || !trust.baselines.contains(&suite.baseline_policy)
        {
            bail!("Validation suite or baseline is not authorized by the external trust root");
        }
        Ok(Self {
            suite,
            trust,
            suite_file,
            trust_file,
        })
    }

    pub fn unchanged(&self, root: &Path) -> Result<()> {
        self.suite_file.unchanged(root)?;
        self.trust_file.unchanged(root)
    }
}

pub fn validate_trust(trust: &EvolutionTrust) -> Result<()> {
    if trust.schema_version != 1
        || !Path::new(&trust.repository).is_absolute()
        || !(1..=31_536_000).contains(&trust.max_age_seconds)
        || !(1..=64).contains(&trust.approval_keys.len())
    {
        bail!("Invalid evolution trust version, repository, approval keys or maximum age");
    }
    for refs in [&trust.suites, &trust.baselines, &trust.evaluators] {
        if refs.is_empty()
            || refs.len() > 256
            || refs.iter().any(|reference| !valid_digest(reference))
            || refs.iter().collect::<BTreeSet<_>>().len() != refs.len()
        {
            bail!(
                "Trust roots require distinct, bounded SHA-256 suite/baseline/evaluator allowlists"
            );
        }
    }
    if trust.revoked_approvals.len() > 4096
        || trust.revoked_approvals.iter().any(|id| !valid_digest(id))
    {
        bail!("Invalid approval revocation list");
    }
    let mut ids = BTreeSet::new();
    let mut actors = BTreeSet::new();
    for key in &trust.approval_keys {
        super::constraints::id(&key.id)?;
        key.actor.validate().map_err(anyhow::Error::msg)?;
        if key.actor.kind != ActorKind::Human
            || !ids.insert(&key.id)
            || !actors.insert(&key.actor.id)
            || key.public_key.is_empty()
            || key.public_key.len() > 128
        {
            bail!("Approval keys require distinct human identities and bounded public keys");
        }
    }
    Ok(())
}

pub fn validate_suite(suite: &ValidationSuite) -> Result<()> {
    super::constraints::id(&suite.id)?;
    super::constraints::id(&suite.evaluator_epoch)?;
    let b = &suite.budget;
    if ![1, 2].contains(&suite.schema_version)
        || !valid_digest(&suite.baseline_policy)
        || !(3..=64).contains(&suite.cases.len())
        || suite.min_improvements > suite.cases.len()
        || !(1..=512).contains(&suite.max_rules)
        || suite.max_rule_growth > suite.max_rules
    {
        bail!("Invalid validation suite schema, baseline, case count or contribution policy");
    }
    if !(1..=1024).contains(&b.snapshot_max_mib)
        || !(1..=8).contains(&b.snapshot_max_file_mib)
        || !(1..=16).contains(&b.snapshot_jobs)
        || !(1..=3600).contains(&b.snapshot_timeout_seconds)
        || !(1..=8).contains(&b.max_parallel)
        || b.max_live_snapshot_mib < b.snapshot_max_mib * 2
        || b.max_live_snapshot_mib > 4096
        || !(1..=3600).contains(&b.case_timeout_seconds)
        || !(1..=7200).contains(&b.total_timeout_seconds)
    {
        bail!("Invalid bounded evaluation resource budget");
    }
    if suite.motivating_evidence.is_empty()
        || suite.motivating_evidence.len() > 128
        || suite
            .motivating_evidence
            .iter()
            .any(|reference| !valid_digest(reference))
    {
        bail!("Suite must bind motivating evidence");
    }
    let mut ids = BTreeSet::new();
    let mut snapshots = BTreeSet::new();
    let mut tasks = BTreeSet::new();
    for case in &suite.cases {
        super::constraints::id(&case.id)?;
        if (suite.schema_version == 2 && case.evidence_ref.is_none())
            || case
                .evidence_ref
                .as_ref()
                .is_some_and(|id| !valid_digest(id))
        {
            bail!("Suite v2 cases require retained evidence digests");
        }
        if !ids.insert(&case.id)
            || !snapshots.insert((&case.base, &case.head))
            || !tasks.insert(&case.task.task_id)
            || case.expectations.is_empty()
            || case.expectations.len() > 512
        {
            bail!(
                "Validation cases require distinct IDs, snapshot pairs, tasks and bounded independent expectations"
            );
        }
        for commit in [&case.base, &case.head] {
            if ![40, 64].contains(&commit.len())
                || !commit
                    .bytes()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
            {
                bail!("Validation snapshots require immutable full Git commit IDs");
            }
        }
        for id in case.expectations.keys() {
            super::constraints::id(id)?;
        }
        super::parse_task(&serde_json::to_vec(&case.task)?)?;
    }
    for kind in [CaseKind::Replay, CaseKind::HeldOut, CaseKind::Anchor] {
        if !suite.cases.iter().any(|case| case.kind == kind) {
            bail!("Validation requires replay, held-out and independent anchor cases");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn native_protected_paths_preserve_confinement_and_read_limits() {
        use super::ProtectedFile;

        let root = tempfile::tempdir().unwrap();
        let external =
            tempfile::tempdir_in(dunce::canonicalize(std::env::temp_dir()).unwrap()).unwrap();
        let nested = external.path().join("nested");
        std::fs::create_dir(&nested).unwrap();
        let path = nested.join("evidence.json");
        std::fs::write(&path, b"evidence").unwrap();
        let file = ProtectedFile::read_limited(root.path(), &path, 8).unwrap();
        assert_eq!(file.bytes, b"evidence");
        file.unchanged(root.path()).unwrap();
        assert!(ProtectedFile::read_limited(root.path(), &path, 7).is_err());
        let traversal = nested.join("..").join("nested").join("evidence.json");
        assert!(ProtectedFile::read(root.path(), &traversal).is_err());
        let internal = root.path().join("evidence.json");
        std::fs::write(&internal, b"evidence").unwrap();
        assert!(ProtectedFile::read(root.path(), &internal).is_err());
        std::fs::write(&path, b"modified").unwrap();
        assert!(file.unchanged(root.path()).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn native_protected_paths_reject_symlink_ancestors() {
        let root = tempfile::tempdir().unwrap();
        let external = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        std::fs::write(target.path().join("evidence.json"), b"evidence").unwrap();
        let link = external.path().join("link");
        std::os::unix::fs::symlink(target.path(), &link).unwrap();
        assert!(super::ProtectedFile::read(root.path(), &link.join("evidence.json")).is_err());
    }

    #[test]
    fn suite_parser_rejects_oversized_and_duplicate_input() {
        let oversized = vec![b' '; crate::config::MAX_CONFIG_BYTES + 1];
        assert!(
            super::parse_suite(&oversized)
                .unwrap_err()
                .to_string()
                .contains("Validation suite exceeds")
        );
        assert!(
            super::parse_suite(b"schema_version: 1\nschema_version: 1\n")
                .unwrap_err()
                .to_string()
                .contains("duplicate")
        );
    }
}
