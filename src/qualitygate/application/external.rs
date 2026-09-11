//! Bounded caller-controlled trust and signed evidence shared by manual and run checks.

use crate::{
    adapters::{attestation, provenance},
    config::{self, CheckKind, Plan, attestation::TrustStore},
    paths,
};
use anyhow::{Result, bail};
use std::{
    collections::BTreeMap,
    io::Read,
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub(super) struct Inputs {
    pub store: TrustStore,
    pub store_path: PathBuf,
    pub store_bytes: Vec<u8>,
    records: BTreeMap<String, Result<RecordInput, String>>,
}

struct RecordInput {
    path: PathBuf,
    limit: usize,
    bytes: Vec<u8>,
}

pub(super) fn requests(plan: &Plan) -> BTreeMap<String, usize> {
    let mut requests = BTreeMap::new();
    for name in plan
        .commands
        .iter()
        .filter(|check| check.kind == CheckKind::Manual)
        .filter_map(|check| check.evidence_file.as_ref())
    {
        requests.insert(name.clone(), attestation::MAX_ENVELOPE_BYTES);
    }
    for rule in plan
        .rules
        .values()
        .filter_map(|rule| rule.provenance.as_ref())
    {
        requests
            .entry(rule.evidence_file.clone())
            .or_insert(provenance::MAX_ENVELOPE_BYTES);
    }
    requests
}

fn read(path: &Path, limit: usize) -> Result<Vec<u8>> {
    if !std::fs::symlink_metadata(path)?.is_file() {
        bail!(
            "External evidence must be a regular non-symlink file: {}",
            path.display()
        );
    }
    let file = std::fs::File::open(path)?;
    if !file.metadata()?.is_file() {
        bail!("External evidence is not a regular file");
    }
    let mut bytes = Vec::new();
    file.take((limit + 1) as u64).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        bail!("External evidence exceeds its byte budget");
    }
    Ok(bytes)
}

pub(super) fn load(
    root: &Path,
    store_path: &Path,
    evidence_directory: &Path,
    requests: &BTreeMap<String, usize>,
) -> Result<Inputs> {
    let started = Instant::now();
    let root = dunce::canonicalize(root)?;
    let canonical_store = dunce::canonicalize(store_path)?;
    let directory = dunce::canonicalize(evidence_directory)?;
    if canonical_store.starts_with(&root) || directory.starts_with(&root) || !directory.is_dir() {
        bail!("Trust store and evidence directory must be outside the checked repository");
    }
    if requests.len() > 128 {
        bail!("External evidence exceeds 128 records");
    }
    let store_bytes = read(store_path, 256 * 1024)?;
    let store = config::attestation::parse(&store_bytes)?;
    attestation::validate_keys(&store)?;
    let mut records = BTreeMap::new();
    let mut total = store_bytes.len();
    for (name, limit) in requests {
        if started.elapsed() > Duration::from_secs(30) {
            bail!("External evidence loading exceeded its 30-second budget");
        }
        let record = (|| -> Result<_> {
            let path = paths::confined(&directory, Path::new(name))?;
            let bytes = read(&path, *limit)?;
            Ok(RecordInput {
                path,
                limit: *limit,
                bytes,
            })
        })()
        .map_err(|error| format!("{error:#}"));
        if let Ok(record) = &record {
            total += record.bytes.len();
        }
        if total > 8 * 1024 * 1024 {
            bail!("External evidence exceeds 8 MiB total");
        }
        records.insert(name.clone(), record);
    }
    Ok(Inputs {
        store,
        store_path: store_path.to_owned(),
        store_bytes,
        records,
    })
}

pub(super) fn record<'a>(inputs: &'a Inputs, name: &str) -> Result<&'a [u8]> {
    match inputs.records.get(name) {
        Some(Ok(record)) => Ok(&record.bytes),
        Some(Err(error)) => bail!("Cannot read external record {name}: {error}"),
        None => bail!("External record is unavailable: {name}"),
    }
}

pub(super) fn now() -> Result<u64> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())
}

pub(super) fn unchanged(inputs: &Inputs) -> Result<()> {
    if read(&inputs.store_path, 256 * 1024)? != inputs.store_bytes {
        bail!("External trust store changed during checks");
    }
    for record in inputs
        .records
        .values()
        .filter_map(|record| record.as_ref().ok())
    {
        if read(&record.path, record.limit)? != record.bytes {
            bail!(
                "External record changed during checks: {}",
                record.path.display()
            );
        }
    }
    Ok(())
}
