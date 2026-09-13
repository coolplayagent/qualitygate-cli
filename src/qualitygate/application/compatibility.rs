//! Execute the same build on both snapshots, then compare immutable produced JARs.

use super::{commands, compatibility_inputs as inputs, tool_evidence};
use crate::{
    adapters::{compatibility, rules::diagnostic},
    config::{CommandCheck, ToolVersion},
    domain::*,
    snapshot::{self, File, Snapshot},
};
use anyhow::{Context, Result, bail};
use std::{collections::BTreeMap, path::Path, sync::Arc};

pub(super) async fn execute(
    check: &CommandCheck,
    artifacts: &Path,
    snapshot: &Arc<Snapshot>,
) -> CheckResult {
    let mut result = CheckResult::pending(&check.id, check.required, check.severity);
    result.applicability = Applicability::Applicable;
    match run(check, artifacts, snapshot, &mut result).await {
        Ok(()) => result.complete(),
        Err(error) => {
            let status = match result.execution.status {
                ExecutionStatus::TimedOut => ExecutionStatus::TimedOut,
                ExecutionStatus::ToolError => ExecutionStatus::ToolError,
                _ => ExecutionStatus::Blocked,
            };
            result.block(
                status,
                format!("Compatibility verification did not complete: {error:#}"),
            );
        }
    }
    result
}

async fn build(
    check: &CommandCheck,
    artifacts: &Path,
    snapshot: &Arc<Snapshot>,
    baseline: bool,
    result: &mut CheckResult,
) -> Result<inputs::Captured> {
    let workspace = snapshot::materialize(snapshot).await?;
    let guard = snapshot::InputGuard::new(workspace.path(), snapshot.files.clone()).await?;
    let spec = check
        .compatibility
        .as_ref()
        .expect("configured compatibility");
    inputs::clear(spec, baseline, workspace.path(), snapshot).await?;
    let mut builder = check.clone();
    builder.compatibility = None;
    builder.id = format!(
        "{}/{}-build",
        check.id,
        if baseline { "baseline" } else { "current" }
    );
    let built = commands::execute(&builder, workspace.path(), artifacts, snapshot, &guard).await;
    result
        .execution
        .artifacts
        .extend(built.execution.artifacts.clone());
    let completed = built.execution.status == ExecutionStatus::Completed
        && built.verdict == Some(Verdict::Pass);
    result.metadata.insert(
        if baseline {
            "baseline_build"
        } else {
            "current_build"
        }
        .into(),
        serde_json::to_value(&built)?,
    );
    if !completed {
        result.execution.status = built.execution.status;
        result.diagnostics.extend(built.diagnostics);
        bail!(
            "{} build did not pass",
            if baseline { "Baseline" } else { "Current" }
        );
    }
    let captured = inputs::collect(
        spec,
        baseline,
        workspace.path(),
        artifacts,
        snapshot,
        result,
    )
    .await?;
    guard.verify().await?;
    Ok(captured)
}

async fn run(
    check: &CommandCheck,
    artifacts: &Path,
    snapshot: &Arc<Snapshot>,
    result: &mut CheckResult,
) -> Result<()> {
    let spec = check
        .compatibility
        .as_ref()
        .context("Missing compatibility configuration")?;
    let analyzer_path = if Path::new(&spec.analyzer_jar).is_absolute() {
        Path::new(&spec.analyzer_jar).to_owned()
    } else {
        snapshot.root.join(&spec.analyzer_jar)
    };
    let analyzer = inputs::read(&analyzer_path).await?;
    let expected = spec.analyzer_sha256.clone();
    let (analyzer, version) = tokio::task::spawn_blocking(move || {
        let version = compatibility::analyzer_version(&analyzer, &expected)?;
        Ok::<_, anyhow::Error>((analyzer, version))
    })
    .await??;
    result.metadata.insert("compatibility".into(),serde_json::json!({
        "tool":"japicmp","tool_version":version,"version_method":"digest_bound_archive_metadata",
        "analyzer_source":analyzer_path,"analyzer_digest":spec.analyzer_sha256,
        "level":spec.level,"source_snapshot":snapshot.identity
    }));
    result.execution.artifacts.push(
        super::evidence::persist(
            artifacts,
            &check.id,
            "compatibility-analyzer.jar",
            &analyzer,
        )
        .await?,
    );
    let source = snapshot.clone();
    let base = tokio::task::spawn_blocking(move || {
        let mut base = source.as_ref().clone();
        base.files = source.base_files.clone();
        base.base_files = base.files.clone();
        base.identity.head = base.identity.base.clone();
        base.identity.mode = "baseline".into();
        base.identity.merge_request = None;
        base.identity.content_digest = snapshot::content_digest(&base.files);
        base.changes.clear();
        base.commits.clear();
        base
    })
    .await?;
    let baseline = build(check, artifacts, &Arc::new(base), true, result).await?;
    let current = build(check, artifacts, snapshot, false, result).await?;
    result
        .metadata
        .get_mut("compatibility")
        .expect("compatibility metadata")["archives"] = serde_json::to_value(
        baseline
            .bindings
            .iter()
            .chain(&current.bindings)
            .collect::<Vec<_>>(),
    )?;
    tool_evidence::comparable(
        &result.metadata["current_build"]["metadata"]["tools"],
        &serde_json::from_value::<Vec<tool_evidence::ToolEvidence>>(
            result.metadata["baseline_build"]["metadata"]["tools"].clone(),
        )?,
    )?;
    if result.metadata["baseline_build"]["metadata"]["command_executable"]["digest"]
        != result.metadata["current_build"]["metadata"]["command_executable"]["digest"]
    {
        bail!("Baseline and current build executables differ");
    }
    let mut files = BTreeMap::from([(
        "analyzer.jar".into(),
        File {
            bytes: analyzer,
            executable: false,
        },
    )]);
    files.extend(baseline.files);
    files.extend(current.files);
    if files.values().map(|file| file.bytes.len()).sum::<usize>() > compatibility::MAX_TOTAL_BYTES {
        bail!("Compatibility inputs exceed 128 MiB including the analyzer");
    }
    let source = snapshot.clone();
    let comparison = tokio::task::spawn_blocking(move || {
        let mut comparison = source.as_ref().clone();
        comparison.identity.mode = "compatibility_inputs".into();
        comparison.identity.content_digest = snapshot::content_digest(&files);
        comparison.files = files;
        comparison.base_files.clear();
        comparison.changes.clear();
        comparison.commits.clear();
        comparison
    })
    .await?;
    let workspace = snapshot::materialize(&comparison).await?;
    let guard = snapshot::InputGuard::new(workspace.path(), comparison.files.clone()).await?;
    let paths = |names: &[String], separator: &str| -> Result<String> {
        names
            .iter()
            .map(|name| {
                workspace
                    .path()
                    .join(name)
                    .to_str()
                    .map(str::to_owned)
                    .context("Compatibility path is not UTF-8")
            })
            .collect::<Result<Vec<_>>>()
            .map(|paths| paths.join(separator))
    };
    let old = paths(&baseline.archives, ";")?;
    let new = paths(&current.archives, ";")?;
    let report_path = workspace.path().join("comparison.xml");
    let mut comparator = check.clone();
    comparator.compatibility = None;
    comparator.cwd = ".".into();
    comparator.required_args.clear();
    comparator.timeout_seconds = spec.timeout_seconds;
    comparator.argv = vec![
        spec.java.clone(),
        "-Xmx512m".into(),
        "-jar".into(),
        workspace
            .path()
            .join("analyzer.jar")
            .to_str()
            .context("Invalid analyzer path")?
            .into(),
        "--old".into(),
        old.clone(),
        "--new".into(),
        new.clone(),
        "--xml-file".into(),
        report_path.to_str().context("Invalid report path")?.into(),
        "-a".into(),
        "private".into(),
        "--include-synthetic".into(),
    ];
    let separator = if cfg!(windows) { ";" } else { ":" };
    for (flag, classpath) in [
        ("--old-classpath", &baseline.classpath),
        ("--new-classpath", &current.classpath),
    ] {
        if !classpath.is_empty() {
            comparator
                .argv
                .extend([flag.into(), paths(classpath, separator)?]);
        }
    }
    comparator.tools = vec![ToolVersion {
        id: "java".into(),
        argv: vec![spec.java.clone(), "-version".into()],
        inputs: vec![],
        timeout_seconds: 30,
    }];
    let comparison = Arc::new(comparison);
    let mut expected_classes = baseline.classes.union(&current.classes).cloned().collect();
    let level = spec.level;
    for (inventory, suffix, access) in [(true, "inventory", "private"), (false, "api", "protected")]
    {
        let report_name = format!("comparison-{suffix}.xml");
        let mut command = comparator.clone();
        command.id = if inventory {
            format!("{}/inventory", check.id)
        } else {
            check.id.clone()
        };
        let output_index = command
            .argv
            .iter()
            .position(|arg| arg == "--xml-file")
            .unwrap()
            + 1;
        command.argv[output_index] = workspace
            .path()
            .join(&report_name)
            .to_str()
            .context("Invalid report path")?
            .into();
        let access_index = command.argv.iter().position(|arg| arg == "-a").unwrap() + 1;
        command.argv[access_index] = access.into();
        let compared =
            commands::execute(&command, workspace.path(), artifacts, &comparison, &guard).await;
        let retained = std::mem::take(&mut result.execution.artifacts);
        result.execution = compared.execution.clone();
        result.execution.artifacts.extend(retained);
        result.metadata.extend(compared.metadata.clone());
        if inventory {
            result.metadata.insert(
                "compatibility_inventory_execution".into(),
                serde_json::to_value(&compared)?,
            );
        }
        result.diagnostics.extend(compared.diagnostics);
        if compared.execution.status != ExecutionStatus::Completed
            || compared.verdict != Some(Verdict::Pass)
        {
            bail!("Compatibility {suffix} analyzer did not complete successfully");
        }
        if !inventory {
            tool_evidence::comparable(
                &result.metadata["tools"],
                &serde_json::from_value::<Vec<tool_evidence::ToolEvidence>>(
                    result.metadata["compatibility_inventory_execution"]["metadata"]["tools"]
                        .clone(),
                )?,
            )?;
        }
        let bytes = super::generated_reports::read_report(workspace.path(), &report_name).await?;
        result.execution.artifacts.push(
            super::evidence::persist(
                artifacts,
                &check.id,
                &format!("compatibility-{suffix}.xml"),
                &bytes,
            )
            .await?,
        );
        let (old, new) = (old.clone(), new.clone());
        let (parsed, next_classes) = tokio::task::spawn_blocking(move || {
            let parsed =
                compatibility::parse(&bytes, &old, &new, &expected_classes, level, inventory)?;
            let next_classes = if inventory {
                compatibility::api_inventory(&bytes)?
            } else {
                expected_classes
            };
            Ok::<_, anyhow::Error>((parsed, next_classes))
        })
        .await??;
        expected_classes = next_classes;
        guard.verify().await?;
        let metadata = result
            .metadata
            .get_mut("compatibility")
            .expect("compatibility metadata");
        if inventory {
            metadata["inventory_classes"] = serde_json::json!(parsed.classes_compared);
            metadata["api_visibility"] = serde_json::json!(["public", "protected"]);
        } else {
            metadata["comparison"] = serde_json::to_value(&parsed)?;
            for finding in parsed.findings {
                result.diagnostics.push(diagnostic(&check.id,None,None,
                    format!("{}: {}",finding.symbol,finding.change),
                    serde_json::to_value(&finding)?,
                    "Restore the compatible API or obtain an approved change to the compatibility contract, then rebuild both versions and recheck",
                    &format!("{}:{}",finding.symbol,finding.change)));
            }
        }
    }
    Ok(())
}
