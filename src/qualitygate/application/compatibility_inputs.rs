//! Fresh bounded build artifacts and explicit analyzer input inventories.

use crate::{
    adapters::compatibility,
    config::compatibility::CompatibilitySpec,
    domain::{Artifact, CheckResult},
    paths,
    snapshot::{File, Snapshot},
};
use anyhow::{Context, Result, bail};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

pub(super) async fn read(path: &Path) -> Result<Vec<u8>> {
    let metadata = tokio::fs::symlink_metadata(path).await.with_context(|| {
        format!(
            "Required compatibility input is missing: {}",
            path.display()
        )
    })?;
    if !metadata.is_file() || metadata.len() > compatibility::MAX_ARCHIVE_BYTES as u64 {
        bail!("Compatibility archive must be a regular non-symlink file of at most 32 MiB");
    }
    use tokio::io::AsyncReadExt;
    let mut bytes = Vec::new();
    tokio::fs::File::open(path)
        .await?
        .take(compatibility::MAX_ARCHIVE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .await?;
    if bytes.len() > compatibility::MAX_ARCHIVE_BYTES {
        bail!("Compatibility archive grew beyond its budget");
    }
    Ok(bytes)
}

async fn confined(root: &Path, name: &str) -> Result<std::path::PathBuf> {
    let root = root.to_owned();
    let name = name.to_owned();
    tokio::task::spawn_blocking(move || paths::confined(&root, Path::new(&name))).await?
}

pub(super) async fn clear(
    spec: &CompatibilitySpec,
    baseline: bool,
    workspace: &Path,
    snapshot: &Snapshot,
) -> Result<()> {
    for pair in spec.artifacts.iter().chain(&spec.classpath) {
        let name = if baseline {
            &pair.baseline
        } else {
            &pair.current
        };
        if snapshot.files.contains_key(name) {
            bail!("Compatibility output would overwrite a checked input: {name}");
        }
        let path = confined(workspace, name).await?;
        match tokio::fs::remove_file(&path).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error).context("Cannot clear a stale compatibility output"),
        }
    }
    Ok(())
}

#[derive(serde::Serialize)]
pub(super) struct Binding {
    pub role: String,
    pub source_path: String,
    pub source_snapshot_digest: String,
    pub input_path: String,
    pub artifact: Artifact,
}

pub(super) struct Captured {
    pub files: BTreeMap<String, File>,
    pub bindings: Vec<Binding>,
    pub classes: BTreeSet<String>,
    pub archives: Vec<String>,
    pub classpath: Vec<String>,
}

pub(super) async fn collect(
    spec: &CompatibilitySpec,
    baseline: bool,
    workspace: &Path,
    artifacts: &Path,
    snapshot: &Snapshot,
    result: &mut CheckResult,
) -> Result<Captured> {
    let side = if baseline { "baseline" } else { "current" };
    let mut captured = Captured {
        files: BTreeMap::new(),
        bindings: vec![],
        classes: BTreeSet::new(),
        archives: vec![],
        classpath: vec![],
    };
    let mut occupied = BTreeSet::new();
    let mut total = 0;
    for (is_api, pairs) in [(true, &spec.artifacts), (false, &spec.classpath)] {
        for (index, pair) in pairs.iter().enumerate() {
            let name = if baseline {
                &pair.baseline
            } else {
                &pair.current
            };
            let bytes = read(&confined(workspace, name).await?).await?;
            total += bytes.len();
            if total > compatibility::MAX_TOTAL_BYTES {
                bail!("Compatibility inputs exceed 128 MiB");
            }
            let (bytes, classes) = tokio::task::spawn_blocking(move || {
                let classes = compatibility::classes(&bytes)?;
                Ok::<_, anyhow::Error>((bytes, classes))
            })
            .await??;
            for class in &classes {
                if !occupied.insert(class.clone()) {
                    bail!("Duplicate class across compatibility inputs: {class}");
                }
            }
            if occupied.len() > 50_000 {
                bail!("Compatibility inputs exceed 50000 classes per version");
            }
            let role = if is_api { "api" } else { "classpath" };
            let input_path = format!("{side}/{role}-{index}.jar");
            let artifact = super::evidence::persist(
                artifacts,
                &result.id,
                &format!("compatibility-{side}-{role}-{index}.jar"),
                &bytes,
            )
            .await?;
            result.execution.artifacts.push(artifact.clone());
            captured.bindings.push(Binding {
                role: role.into(),
                source_path: name.clone(),
                source_snapshot_digest: snapshot.identity.content_digest.clone(),
                input_path: input_path.clone(),
                artifact,
            });
            captured.files.insert(
                input_path.clone(),
                File {
                    bytes,
                    executable: false,
                },
            );
            if is_api {
                captured.classes.extend(classes);
                captured.archives.push(input_path);
            } else {
                captured.classpath.push(input_path);
            }
        }
    }
    Ok(captured)
}
