//! Bind native parser loading to a private, preprovisioned, download-disabled cache.

use crate::{
    domain::Artifact,
    snapshot::Snapshot,
    tool_evidence::contracts::{LANGUAGE_PACK_VERSION, MAX_FACT_BYTES, OutputSpec, ToolFormat},
};
use anyhow::{Context, Result, ensure};
use std::{collections::BTreeMap, path::Path};

const DENIED_MANIFEST: &[u8] = b"Qualitygate disables parser downloads.\n";
const PARSER_NAME: &str = "libtree_sitter_rust.so";

pub(crate) struct PreparedRuntime {
    pub env: BTreeMap<String, String>,
    pub artifacts: Vec<Artifact>,
    directory: tempfile::TempDir,
    expected: Vec<Artifact>,
}

/// Source bytes come from the selected snapshot, never the mutable working tree.
/// Parser installation and language-pack installation remain external preparation.
pub(crate) async fn prepare(
    spec: &OutputSpec,
    snapshot: &Snapshot,
    artifacts_dir: &Path,
    producer: &str,
) -> Result<Option<PreparedRuntime>> {
    if spec.adapter == ToolFormat::ToolFacts {
        return Ok(None);
    }
    ensure!(
        cfg!(all(target_os = "linux", target_arch = "x86_64")),
        "Native runtime profile currently supports Linux x86_64 only"
    );
    spec.validate()?;
    let runtime = spec
        .native_runtime
        .as_ref()
        .context("Native execution requires a preprovisioned parser runtime")?;
    let parser = snapshot
        .files
        .get(&runtime.parser_path)
        .context("Native parser is absent from the selected snapshot")?;
    ensure!(
        !parser.bytes.is_empty() && parser.bytes.len() <= MAX_FACT_BYTES,
        "Native parser exceeds its 32 MiB input bound or is empty"
    );
    let bytes = parser.bytes.clone();
    let declared_digest = runtime.parser_digest.clone();
    tokio::task::spawn_blocking(move || {
        ensure!(
            crate::content_digest::sha256(&bytes) == declared_digest,
            "Native parser digest does not match selected snapshot bytes"
        );
        Ok::<_, anyhow::Error>(())
    })
    .await??;
    Ok(Some(
        prepare_cache(&parser.bytes, artifacts_dir, producer).await?,
    ))
}

async fn prepare_cache(
    parser: &[u8],
    artifacts_dir: &Path,
    producer: &str,
) -> Result<PreparedRuntime> {
    let directory = tokio::task::spawn_blocking(|| {
        tempfile::Builder::new()
            .prefix("qualitygate-native-runtime-")
            .tempdir()
    })
    .await??;
    let version_dir = directory
        .path()
        .join("tree-sitter-language-pack")
        .join(format!("v{LANGUAGE_PACK_VERSION}"));
    let libs = version_dir.join("libs");
    tokio::fs::create_dir_all(&libs).await?;
    // The upstream loader rejects group/other-writable cache directories. Set each
    // level explicitly instead of relying on the caller's umask.
    #[cfg(unix)]
    for path in [
        directory.path(),
        version_dir.parent().unwrap(),
        &version_dir,
        &libs,
    ] {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).await?;
    }
    let parser_path = libs.join(PARSER_NAME);
    let manifest_path = directory.path().join("disabled-manifest.json");
    write_new(&parser_path, parser).await?;
    write_new(&manifest_path, DENIED_MANIFEST).await?;
    let expected = vec![
        crate::runner::identity::evidence(&parser_path).await?,
        crate::runner::identity::evidence(&manifest_path).await?,
    ];
    let artifacts = vec![
        crate::application::evidence::persist(
            artifacts_dir,
            producer,
            "native-rust-parser.so",
            parser,
        )
        .await?,
        crate::application::evidence::persist(
            artifacts_dir,
            producer,
            "native-disabled-manifest.json",
            DENIED_MANIFEST,
        )
        .await?,
    ];
    // A fresh cache has no cached manifest or bundle. The pinned pack therefore
    // loads this verified library, or reads invalid local JSON and fails before
    // any remote URL is available. LIBS_DIR also replaces ambient parser paths.
    let env = BTreeMap::from([
        (
            "TREE_SITTER_LANGUAGE_PACK_CACHE_DIR".into(),
            utf8(directory.path())?,
        ),
        ("TREE_SITTER_LANGUAGE_PACK_LIBS_DIR".into(), utf8(&libs)?),
        (
            "TREE_SITTER_LANGUAGE_PACK_MANIFEST_URL".into(),
            format!("file://{}", utf8(&manifest_path)?),
        ),
    ]);
    let prepared = PreparedRuntime {
        env,
        artifacts,
        directory,
        expected,
    };
    verify(Some(&prepared)).await?;
    Ok(prepared)
}

fn utf8(path: &Path) -> Result<String> {
    Ok(path
        .to_str()
        .context("Native cache path is not UTF-8")?
        .into())
}

async fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    use tokio::io::AsyncWriteExt;
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .await?;
    file.write_all(bytes).await?;
    file.flush().await?;
    Ok(())
}

pub(crate) async fn verify(prepared: Option<&PreparedRuntime>) -> Result<()> {
    let Some(prepared) = prepared else {
        return Ok(());
    };
    for artifact in &prepared.expected {
        let metadata = tokio::fs::symlink_metadata(&artifact.path).await?;
        ensure!(
            metadata.is_file(),
            "Native runtime input is no longer a regular file"
        );
        let actual = crate::runner::identity::evidence(Path::new(&artifact.path)).await?;
        ensure!(
            actual.digest == artifact.digest && actual.bytes == artifact.bytes,
            "Native producer modified its parser or disabled manifest"
        );
    }
    let version_dir = prepared
        .directory
        .path()
        .join("tree-sitter-language-pack")
        .join(format!("v{LANGUAGE_PACK_VERSION}"));
    // A missing-parser failure may leave the pack's empty lock file. No other
    // data files, symlinks, cached manifests or bundles may be introduced.
    for (path, allowed) in [
        (
            prepared.directory.path().to_owned(),
            vec!["disabled-manifest.json", "tree-sitter-language-pack"],
        ),
        (version_dir.parent().unwrap().to_owned(), vec!["v1.25.2"]),
        (version_dir.clone(), vec!["libs", ".download.lock"]),
        (version_dir.join("libs"), vec![PARSER_NAME]),
    ] {
        let metadata = tokio::fs::symlink_metadata(&path).await?;
        ensure!(metadata.is_dir(), "Native runtime directory was replaced");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            ensure!(
                metadata.permissions().mode() & 0o077 == 0,
                "Native runtime directory is not private"
            );
        }
        let mut entries = tokio::fs::read_dir(path).await?;
        while let Some(entry) = entries.next_entry().await? {
            let name = entry.file_name();
            ensure!(
                name.to_str().is_some_and(|name| allowed.contains(&name)),
                "Native runtime introduced an unexpected cache entry"
            );
            let metadata = tokio::fs::symlink_metadata(entry.path()).await?;
            ensure!(
                !metadata.is_symlink(),
                "Native runtime introduced a symlink"
            );
            if name == ".download.lock" {
                ensure!(
                    metadata.is_file() && metadata.len() == 0,
                    "Invalid native cache lock"
                );
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "native_runtime_tests.rs"]
mod tests;
