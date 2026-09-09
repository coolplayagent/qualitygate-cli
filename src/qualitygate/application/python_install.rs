//! Collect bounded, durable installed metadata from the command's fresh target.

use crate::{
    config::{CommandCheck, PythonProject},
    domain::{CheckResult, ProjectFacts},
    paths,
    snapshot::{self, Snapshot},
};
use anyhow::{Context, Result, bail};
use std::{
    collections::BTreeMap,
    io::Read,
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};

pub(super) async fn collect(
    check: &CommandCheck,
    project: &PythonProject,
    workspace: &Path,
    artifacts: &Path,
    snapshot: &Arc<Snapshot>,
    result: &mut CheckResult,
    index: usize,
) -> Result<ProjectFacts> {
    let report = super::generated_reports::read_report(workspace, &project.install_report).await?;
    result.execution.artifacts.push(
        super::evidence::persist(
            artifacts,
            &check.id,
            &format!("project-{index}-install"),
            &report,
        )
        .await?,
    );
    let root = workspace.to_owned();
    let target = project.install_target.clone();
    let metadata = tokio::task::spawn_blocking(move || read_metadata(&root, &target)).await??;
    for (number, (_, bytes)) in metadata.iter().enumerate() {
        result.execution.artifacts.push(
            super::evidence::persist(
                artifacts,
                &check.id,
                &format!("project-{index}-metadata-{number}"),
                bytes,
            )
            .await?,
        );
    }
    let versions: BTreeMap<String, String> = result
        .metadata
        .get("tools")
        .and_then(serde_json::Value::as_array)
        .context("Python tool versions are missing")?
        .iter()
        .map(|tool| {
            Ok((
                tool["id"].as_str().context("Missing tool ID")?.into(),
                tool["version"]
                    .as_str()
                    .context("Missing tool version")?
                    .into(),
            ))
        })
        .collect::<Result<_>>()?;
    let project = project.clone();
    let workspace = workspace.to_owned();
    let snapshot = Arc::clone(snapshot);
    let producer = check.id.clone();
    tokio::task::spawn_blocking(move || {
        crate::adapters::python::parse(
            &project, &report, &metadata, &workspace, &snapshot, &producer, &versions,
        )
    })
    .await?
}

fn read_metadata(workspace: &Path, target: &str) -> Result<Vec<(String, Vec<u8>)>> {
    let target = paths::confined(workspace, Path::new(target))?;
    let started = Instant::now();
    let mut metadata = Vec::new();
    let mut total = 0;
    for (index, entry) in std::fs::read_dir(target)?.enumerate() {
        if index > 20_000 || started.elapsed() > Duration::from_secs(30) {
            bail!("Python installed metadata scan exceeded budget");
        }
        let entry = entry?;
        if !entry.file_name().to_string_lossy().ends_with(".dist-info") {
            continue;
        }
        let relative = paths::from_native(entry.path().join("METADATA").strip_prefix(workspace)?)?;
        let path = paths::confined(workspace, Path::new(&relative))?;
        let mut bytes = Vec::new();
        std::fs::File::open(path)?
            .take(snapshot::MAX_FILE_BYTES as u64 + 1)
            .read_to_end(&mut bytes)?;
        total += bytes.len();
        if bytes.len() > snapshot::MAX_FILE_BYTES
            || total > snapshot::MAX_SNAPSHOT_BYTES
            || metadata.len() >= 4096
        {
            bail!("Installed Python metadata exceeds size budget");
        }
        metadata.push((relative, bytes));
    }
    if metadata.is_empty() {
        bail!("Python installation target has no distribution metadata");
    }
    metadata.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(metadata)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn preparation_creates_report_parent_and_rejects_existing_installation() {
        let root = tempfile::tempdir().unwrap();
        let check:CommandCheck=serde_json::from_value(serde_json::json!({"id":"python","argv":["python"],"projects":[{"ecosystem":"python","root":".","source_root":"src","test_source_root":"tests","install_target":"target/python","install_report":"target/pip.json"}]})).unwrap();
        super::super::generated_reports::prepare(&check, root.path(), false)
            .await
            .unwrap();
        assert!(root.path().join("target").is_dir());
        assert!(!root.path().join("target/python").exists());
        std::fs::write(root.path().join("target/pip.json"), "stale").unwrap();
        super::super::generated_reports::prepare(&check, root.path(), false)
            .await
            .unwrap();
        assert!(!root.path().join("target/pip.json").exists());
        std::fs::create_dir_all(root.path().join("target/python")).unwrap();
        assert!(
            super::super::generated_reports::prepare(&check, root.path(), false)
                .await
                .is_err()
        );
        assert!(root.path().join("target/python").is_dir());
    }

    #[test]
    fn installed_metadata_inventory_is_bounded_and_requires_real_files() {
        let root = tempfile::tempdir().unwrap();
        assert!(read_metadata(root.path(), "target/python").is_err());
        std::fs::create_dir_all(root.path().join("target/python/pkg")).unwrap();
        assert!(read_metadata(root.path(), "target/python").is_err());
        let info = root.path().join("target/python/pkg.dist-info");
        std::fs::create_dir_all(&info).unwrap();
        assert!(read_metadata(root.path(), "target/python").is_err());
        std::fs::write(
            info.join("METADATA"),
            "Metadata-Version: 2.4\nName: pkg\nVersion: 1\n",
        )
        .unwrap();
        let inventory = read_metadata(root.path(), "target/python").unwrap();
        assert_eq!(inventory.len(), 1);
        assert_eq!(inventory[0].0, "target/python/pkg.dist-info/METADATA");
        std::fs::write(
            info.join("METADATA"),
            vec![b'x'; snapshot::MAX_FILE_BYTES + 1],
        )
        .unwrap();
        assert!(read_metadata(root.path(), "target/python").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn installed_metadata_symlinks_cannot_escape_the_materialized_workspace() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("target/python")).unwrap();
        std::fs::write(outside.path().join("METADATA"), "private").unwrap();
        std::os::unix::fs::symlink(
            outside.path(),
            root.path().join("target/python/pkg.dist-info"),
        )
        .unwrap();
        assert!(read_metadata(root.path(), "target/python").is_err());
    }
}
