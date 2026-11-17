//! Typed coverage observations retained only after the normal report gate validates them.
use crate::{
    config::{IncrementMode, ReportSpec},
    report_evidence::{report_sources::Sources, tool_reports::Data},
    snapshot::Snapshot,
    tool_evidence::contracts::CoverageEvidence,
};
use anyhow::{Result, bail, ensure};
use std::{
    collections::BTreeMap,
    path::Path,
    time::{Duration, Instant},
};

pub fn collect(
    data: &Data,
    spec: &ReportSpec,
    snapshot: &Snapshot,
    workspace: &Path,
    producer: &str,
    bytes: &[u8],
) -> Result<Option<CoverageEvidence>> {
    if spec.coverage_paths.is_empty() || spec.mode != IncrementMode::Full {
        return Ok(None);
    }
    let mut sources = Sources::new(snapshot, workspace);
    let mut lines: BTreeMap<String, BTreeMap<usize, u64>> = BTreeMap::new();
    let started = Instant::now();
    let mut remaining = 32 * 1024 * 1024usize;
    ensure!(
        data.coverage.len() <= 50_000,
        "Typed coverage inventory exceeds 50000 records"
    );
    for record in &data.coverage {
        ensure!(
            started.elapsed() < Duration::from_secs(30),
            "Typed coverage normalization exceeded 30 seconds"
        );
        if record.excluded {
            continue;
        }
        let file = crate::report_evidence::coverage_gate::map_source(
            &record.file,
            data,
            snapshot,
            workspace,
            &mut sources,
        )?;
        remaining = remaining
            .checked_sub(file.len().saturating_mul(6).saturating_add(128))
            .ok_or_else(|| anyhow::anyhow!("Typed coverage exceeds 32 MiB"))?;
        if lines
            .entry(file)
            .or_default()
            .insert(record.line, record.hits)
            .is_some()
        {
            bail!("Duplicate typed coverage line");
        }
    }
    Ok(Some(CoverageEvidence {
        producer: producer.into(),
        snapshot_digest: snapshot.identity.content_digest.clone(),
        artifact_digests: vec![crate::snapshot::digest(bytes)],
        inputs: if spec.from_stdout {
            BTreeMap::new()
        } else {
            BTreeMap::from([(spec.path.clone(), crate::snapshot::digest(bytes))])
        },
        lines,
    }))
}

pub fn merge(target: &mut Option<CoverageEvidence>, next: CoverageEvidence) -> Result<()> {
    let Some(current) = target else {
        *target = Some(next);
        return Ok(());
    };
    for (file, lines) in next.lines {
        let destination = current.lines.entry(file).or_default();
        for (line, hits) in lines {
            if destination.insert(line, hits).is_some() {
                bail!("Coverage producers cannot repeat a line across reports");
            }
        }
    }
    current.artifact_digests.extend(next.artifact_digests);
    current.inputs.extend(next.inputs);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report_evidence::tool_reports::CoverageLine;
    #[test]
    fn typed_coverage_keeps_zero_hits_separate_from_missing_and_preserves_input_identity() {
        let root = tempfile::tempdir().unwrap();
        let mut snapshot = crate::application::test_support::snapshot(root.path());
        snapshot.files.insert(
            "lib.rs".into(),
            crate::snapshot::File {
                bytes: b"fn a() {}\n".to_vec(),
                executable: false,
            },
        );
        let mut spec:ReportSpec = serde_json::from_value(serde_json::json!({"path":"coverage/lcov.info","format":"lcov","coverage_paths":["*.rs"],"minimum_coverage":0})).unwrap();
        let data = Data {
            coverage: vec![CoverageLine {
                file: "lib.rs".into(),
                line: 1,
                hits: 0,
                branches_found: 0,
                branches_hit: 0,
                excluded: false,
            }],
            ..Default::default()
        };
        let observed = collect(&data, &spec, &snapshot, root.path(), "coverage", b"raw")
            .unwrap()
            .unwrap();
        assert_eq!(observed.lines["lib.rs"][&1], 0);
        assert!(!observed.lines["lib.rs"].contains_key(&2));
        assert_eq!(
            observed.inputs["coverage/lcov.info"],
            crate::snapshot::digest(b"raw")
        );
        let mut merged = None;
        merge(&mut merged, observed.clone()).unwrap();
        assert!(merge(&mut merged, observed).is_err());
        spec.mode = IncrementMode::ChangedLines;
        assert!(
            collect(&data, &spec, &snapshot, root.path(), "coverage", b"raw")
                .unwrap()
                .is_none()
        );
    }
}
