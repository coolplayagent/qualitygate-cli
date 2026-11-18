//! Complete-time evidence checks and transitive invalidation of fact consumers.
use super::rule_execution::Context as RuleContext;
use crate::{
    config::{CommandCheck, Plan},
    domain::{CheckResult, ExecutionStatus},
    paths,
    tool_evidence::contracts::{CoverageEvidence, ToolFormat},
};
use anyhow::{Context, Result, bail, ensure};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    time::{Duration, Instant},
};

pub(crate) async fn coverage_inputs(
    check: &CommandCheck,
    workspace: &Path,
    coverage: Option<&CoverageEvidence>,
) -> Result<()> {
    let Some(spec) = &check.tool_facts else {
        return Ok(());
    };
    if spec.adapter != ToolFormat::Crapper {
        return Ok(());
    }
    let coverage = coverage
        .context("Crapper coverage prerequisite has no independently validated coverage")?;
    ensure!(
        spec.coverage_producer.as_ref() == Some(&coverage.producer),
        "Coverage producer identity differs"
    );
    ensure!(
        !coverage.inputs.is_empty(),
        "Crapper needs independently produced coverage files, not stdout alone"
    );
    let workspace = workspace.to_owned();
    let expected = coverage.inputs.clone();
    tokio::task::spawn_blocking(move || {
        let started = Instant::now();
        let mut pending = Vec::new();
        for root in ["target/coverage", "coverage"] {
            let path = paths::confined(&workspace, Path::new(root))?;
            match std::fs::symlink_metadata(&path) {
                Ok(metadata) => {
                    ensure!(metadata.is_dir() && !metadata.file_type().is_symlink(), "Coverage scan root must be an ordinary directory");
                    pending.push(path);
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {},
                Err(error) => return Err(error.into()),
            }
        }
        let mut observed = BTreeSet::new();
        let mut visited = 0;
        while let Some(directory) = pending.pop() {
            for entry in std::fs::read_dir(directory)? {
                visited += 1;
                ensure!(visited <= 50_000 && started.elapsed() < Duration::from_secs(30), "Crapper coverage scan exceeded budget");
                let entry = entry?;
                let file_type = entry.file_type()?;
                ensure!(!file_type.is_symlink(), "Crapper coverage scan cannot include symlinks");
                if file_type.is_dir() { pending.push(entry.path()); continue; }
                ensure!(file_type.is_file(), "Coverage scan contains a special file");
                if entry.path().extension().is_some_and(|ext| ext == "html") {
                    bail!("Unverified HTML coverage would override the independently validated LCOV input");
                }
                if entry.file_name() == "lcov.info" {
                    let path = paths::from_native(entry.path().strip_prefix(&workspace)?)?;
                    observed.insert(path);
                }
            }
        }
        ensure!(observed == expected.keys().cloned().collect(), "Crapper discovered coverage inputs differ from the independently verified inventory");
        let mut remaining = 32 * 1024 * 1024u64;
        for (path, digest) in expected {
            ensure!(started.elapsed() < Duration::from_secs(30), "Coverage input verification exceeded 30 seconds");
            let path = paths::confined(&workspace, Path::new(&path))?;
            let metadata = std::fs::metadata(&path)?;
            ensure!(metadata.is_file() && metadata.len() <= 32 * 1024 * 1024, "Coverage input is not a bounded regular file");
            remaining = remaining.checked_sub(metadata.len()).context("Coverage input inventory exceeds 32 MiB")?;
            use std::io::Read;
            let mut bytes = Vec::new();
            std::fs::File::open(path)?.take(32 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
            ensure!(bytes.len() <= 32 * 1024 * 1024 && crate::snapshot::digest(&bytes) == digest, "Independent coverage input changed before or during crapper execution");
        }
        Ok(())
    }).await?
}

pub(crate) async fn finalize(
    plan: &Plan,
    context: &RuleContext,
    workspace: Option<&Path>,
    results: &mut [CheckResult],
    invalid_inputs: bool,
) {
    let mut invalid = BTreeMap::new();
    for (id, receipt) in &context.receipts {
        let proof = async {
            ensure!(
                !invalid_inputs,
                "Snapshot, policy or execution inputs failed completion-time verification"
            );
            let workspace = workspace.context("Producer workspace is unavailable")?;
            let check = plan
                .commands
                .iter()
                .find(|check| &check.id == id)
                .context("Producer plan is unavailable")?;
            let cwd = paths::confined(workspace, Path::new(&check.cwd))?;
            let actual = crate::runner::identity::executable(&check.argv[0], &cwd).await?;
            ensure!(
                actual.digest == receipt.command.digest && actual.path == receipt.command.path,
                "Producer executable changed after fact collection"
            );
            super::tool_evidence::verify(check, workspace, &receipt.tools).await?;
            for artifact in &receipt.artifacts {
                let actual = crate::runner::identity::evidence(Path::new(&artifact.path)).await?;
                ensure!(
                    actual.digest == artifact.digest && actual.bytes == artifact.bytes,
                    "Retained producer evidence changed: {}",
                    artifact.path
                );
            }
            Ok::<_, anyhow::Error>(())
        }
        .await;
        if let Err(error) = proof {
            invalid.insert(
                id.clone(),
                format!("Fact evidence invalid at completion: {error:#}"),
            );
        }
    }
    propagate(plan, results, &mut invalid);
}

fn propagate(plan: &Plan, results: &mut [CheckResult], invalid: &mut BTreeMap<String, String>) {
    for id in &plan.order {
        let dependencies = plan.rules.get(id).map(|rule| &rule.depends_on).or_else(|| {
            plan.commands
                .iter()
                .find(|command| &command.id == id)
                .map(|command| &command.depends_on)
        });
        if let Some(dependency) = dependencies
            .and_then(|dependencies| dependencies.iter().find(|id| invalid.contains_key(*id)))
        {
            invalid.entry(id.clone()).or_insert_with(|| {
                format!("Prerequisite {dependency} evidence invalid at completion")
            });
        }
        if let Some(reason) = invalid.get(id)
            && let Some(result) = results.iter_mut().find(|result| &result.id == id)
        {
            result.block(ExecutionStatus::Blocked, reason.clone());
            result
                .metadata
                .insert("facts_valid_at_completion".into(), false.into());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn crapper_check() -> CommandCheck {
        serde_json::from_value(serde_json::json!({
            "id": "crapper",
            "argv": ["crapper", "--use-existing-coverage", "src/lib.rs"],
            "depends_on": ["coverage"],
            "tool_facts": {
                "adapter": "crapper",
                "schema": "function_metric",
                "path": "facts.edn",
                "tool_id": "crapper",
                "tool_version": crate::tool_evidence::contracts::CRAPPER_VERSION,
                "scope": {"project": "sample"},
                "paths": ["src/lib.rs"],
                "coverage_producer": "coverage"
            }
        }))
        .unwrap()
    }

    #[tokio::test]
    async fn crapper_checks_discovered_coverage_inventory_and_contents() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path();
        let check = crapper_check();
        let paths = ["target/coverage/rust/lcov.info", "coverage/lcov.info"];
        let contents = b"SF:src/lib.rs\nDA:1,1\nend_of_record\n";
        let mut inputs = BTreeMap::new();
        for path in paths {
            let file = root.join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, contents).unwrap();
            inputs.insert(path.into(), crate::snapshot::digest(contents));
        }
        let coverage = CoverageEvidence {
            producer: "coverage".into(),
            snapshot_digest: crate::snapshot::digest(b"sample snapshot"),
            artifact_digests: vec![crate::snapshot::digest(contents)],
            lines: BTreeMap::new(),
            inputs,
        };

        coverage_inputs(&check, root, Some(&coverage))
            .await
            .unwrap();

        let changed = root.join(paths[1]);
        std::fs::write(&changed, b"SF:src/lib.rs\nDA:1,0\nend_of_record\n").unwrap();
        let error = coverage_inputs(&check, root, Some(&coverage))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("coverage input changed"));
        std::fs::write(changed, contents).unwrap();

        let additional = root.join("coverage/unverified");
        std::fs::create_dir(&additional).unwrap();
        let extra_lcov = additional.join("lcov.info");
        std::fs::write(&extra_lcov, contents).unwrap();
        let error = coverage_inputs(&check, root, Some(&coverage))
            .await
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("differ from the independently verified inventory")
        );
        std::fs::remove_file(extra_lcov).unwrap();

        let html = additional.join("report.html");
        std::fs::write(&html, "<html></html>").unwrap();
        let error = coverage_inputs(&check, root, Some(&coverage))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("Unverified HTML coverage"));
        std::fs::remove_file(html).unwrap();

        coverage_inputs(&check, root, Some(&coverage))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn crapper_requires_file_evidence_from_its_declared_coverage_producer() {
        let root = tempfile::tempdir().unwrap();
        let check = crapper_check();
        let mut coverage = CoverageEvidence {
            producer: "other".into(),
            snapshot_digest: crate::snapshot::digest(b"sample snapshot"),
            artifact_digests: vec![crate::snapshot::digest(b"stdout coverage report")],
            lines: BTreeMap::new(),
            inputs: BTreeMap::new(),
        };
        let error = coverage_inputs(&check, root.path(), None)
            .await
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("no independently validated coverage")
        );

        let error = coverage_inputs(&check, root.path(), Some(&coverage))
            .await
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Coverage producer identity differs")
        );

        coverage.producer = "coverage".into();
        let error = coverage_inputs(&check, root.path(), Some(&coverage))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("not stdout alone"));

        std::fs::write(root.path().join("coverage"), "not a directory").unwrap();
        coverage.inputs.insert(
            "coverage/lcov.info".into(),
            crate::snapshot::digest(b"coverage report"),
        );
        let error = coverage_inputs(&check, root.path(), Some(&coverage))
            .await
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Coverage scan root must be an ordinary directory")
        );
    }

    #[test]
    fn final_invalidation_reaches_transitive_consumers_without_turning_failures_into_violations() {
        let config = crate::config::parse(b"schema_version: 1\nchecks:\n - {id: producer, argv: [tool]}\n - {id: consumer, argv: [tool], depends_on: [producer]}\n - {id: dependent, argv: [tool], depends_on: [consumer]}\n").unwrap();
        let plan = Plan::build(&config, None, "full").unwrap();
        let mut results: Vec<_> = plan
            .order
            .iter()
            .map(|id| {
                let mut result = CheckResult::pending(id, true, crate::domain::Severity::Error);
                result.complete();
                result
            })
            .collect();
        propagate(
            &plan,
            &mut results,
            &mut BTreeMap::from([("producer".into(), "artifact changed".into())]),
        );
        assert!(
            results
                .iter()
                .all(|result| result.execution.status == ExecutionStatus::Blocked
                    && result.verdict.is_none())
        );
    }
}
