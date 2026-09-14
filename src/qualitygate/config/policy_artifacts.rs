//! Retain evidence bytes by digest so approvals and history survive external run-directory cleanup.

use super::{
    policy_acceptance::ProtectedFile,
    policy_store::{MAX_OBJECT_BYTES, Store, digest},
};
use crate::domain::{Artifact, Report};
use anyhow::{Result, bail};
use std::{collections::BTreeMap, path::Path};

fn artifacts(report: &Report) -> Result<BTreeMap<String, Artifact>> {
    let mut artifacts = BTreeMap::new();
    for check in &report.checks {
        for artifact in &check.execution.artifacts {
            insert(&mut artifacts, artifact.clone())?;
        }
        for key in ["tools", "baseline_tools"] {
            for tool in check
                .metadata
                .get(key)
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
            {
                for stream in ["stdout", "stderr"] {
                    if let Some(value) = tool.get(stream) {
                        let artifact: Artifact = serde_json::from_value(value.clone())?;
                        insert(&mut artifacts, artifact)?;
                    }
                }
            }
        }
    }
    Ok(artifacts)
}

fn insert(artifacts: &mut BTreeMap<String, Artifact>, artifact: Artifact) -> Result<()> {
    if artifacts
        .get(&artifact.digest)
        .is_some_and(|prior| prior.bytes != artifact.bytes)
    {
        bail!("Duplicate artifact digest has inconsistent byte counts");
    }
    artifacts.insert(artifact.digest.clone(), artifact);
    if artifacts.len() > 4096 {
        bail!("Report artifact inventory exceeds 4096 objects");
    }
    Ok(())
}

pub fn retain(root: &Path, store: &mut Store, report: &Report) -> Result<()> {
    for artifact in artifacts(report)?.into_values() {
        let file = ProtectedFile::read_limited(root, artifact.path.as_ref(), MAX_OBJECT_BYTES)?;
        if file.bytes.len() as u64 != artifact.bytes || digest(&file.bytes) != artifact.digest {
            bail!(
                "Execution evidence changed before archive retention: {}",
                artifact.path
            );
        }
        store.put_blob(&file.bytes)?;
    }
    Ok(())
}

pub fn verify(store: &Store, report: &Report) -> Result<()> {
    for artifact in artifacts(report)?.into_values() {
        if store.blob(&artifact.digest)?.len() as u64 != artifact.bytes {
            bail!("Retained execution evidence has a mismatched byte count");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::*;
    use serde_json::json;

    fn report(artifact: Artifact) -> Report {
        let mut check = CheckResult::pending("producer", true, Severity::Error);
        check.complete();
        check.execution.artifacts.push(artifact.clone());
        check
            .metadata
            .insert("tools".into(), json!([{"stdout":artifact}]));
        serde_json::from_value(json!({"schema_version":1,"run_id":"test","scope":"task","profile":"full",
            "snapshot":{"mode":"diff","base":"base","head":"head","content_digest":"snapshot"},
            "policy":{"source":"policy","config_digest":"config","rules_digest":"rules","task_contract_digest":null,"trust":"test","changes":[]},
            "plan":{"required_checks":["producer"],"pending_delivery_checks":[],"acceptance":{}},
            "gate":{"complete":true,"decision":"pass","blockers":[]},"checks":[check],"summary":Summary::default()})).unwrap()
    }

    #[test]
    fn large_logs_survive_external_cleanup_and_mismatched_counts_cannot_be_approved() {
        let root = tempfile::tempdir().unwrap();
        let external =
            tempfile::tempdir_in(dunce::canonicalize(std::env::temp_dir()).unwrap()).unwrap();
        let bytes = vec![b'x'; 2 * 1024 * 1024];
        let path = external.path().join("stdout.log");
        std::fs::write(&path, &bytes).unwrap();
        let mut report = report(Artifact {
            path: path.to_str().unwrap().into(),
            digest: digest(&bytes),
            bytes: bytes.len() as u64,
        });
        Store::transaction(root.path(), |store| retain(root.path(), store, &report)).unwrap();
        std::fs::remove_file(&path).unwrap();
        let store = Store::open(root.path()).unwrap();
        verify(&store, &report).unwrap();
        assert_eq!(store.blob(&digest(&bytes)).unwrap(), bytes);
        report.checks[0].execution.artifacts[0].bytes += 1;
        assert!(verify(&store, &report).is_err());
        report.checks[0].metadata.clear();
        assert!(verify(&store, &report).is_err());
    }

    #[test]
    fn changed_or_repository_owned_logs_never_publish_evidence() {
        let root = tempfile::tempdir().unwrap();
        let external =
            tempfile::tempdir_in(dunce::canonicalize(std::env::temp_dir()).unwrap()).unwrap();
        let path = external.path().join("stdout.log");
        std::fs::write(&path, b"edited").unwrap();
        let mut report = report(Artifact {
            path: path.to_str().unwrap().into(),
            digest: digest(b"original"),
            bytes: 8,
        });
        assert!(
            Store::transaction(root.path(), |store| retain(root.path(), store, &report)).is_err()
        );
        assert!(Store::optional(root.path()).unwrap().is_none());
        let path = root.path().join("stdout.log");
        std::fs::write(&path, b"original").unwrap();
        report.checks[0].metadata.clear();
        report.checks[0].execution.artifacts[0].path = path.to_str().unwrap().into();
        assert!(
            Store::transaction(root.path(), |store| retain(root.path(), store, &report)).is_err()
        );
    }
}
