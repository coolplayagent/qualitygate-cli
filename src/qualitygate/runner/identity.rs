//! Identify the executable selected from the command's actual working directory.

use crate::domain::Artifact;
use anyhow::{Result, bail};
use sha2::{Digest, Sha256};
use std::{io::Read, path::Path};

const MAX_EXECUTABLE_BYTES: u64 = 256 * 1024 * 1024;

pub async fn executable(program: &str, cwd: &Path) -> Result<Artifact> {
    let program = program.to_owned();
    let cwd = cwd.to_owned();
    tokio::task::spawn_blocking(move || {
        let path = crate::env::executable(&program, &cwd)?;
        identify(&path, MAX_EXECUTABLE_BYTES)
    })
    .await?
}

/// Development evaluators can include large debug sections; this separate limit
/// does not relax the configured producer executable's 256 MiB boundary.
pub async fn evaluator() -> Result<Artifact> {
    tokio::task::spawn_blocking(|| identify(&crate::env::current_executable()?, 1024 * 1024 * 1024))
        .await?
}

/// Pin a caller-selected Qualitygate evaluator with the same bounded debug allowance.
pub async fn selected_evaluator(path: &Path) -> Result<Artifact> {
    let path = path.to_owned();
    tokio::task::spawn_blocking(move || identify(&path, 1024 * 1024 * 1024)).await?
}

fn identify(path: &Path, limit: u64) -> Result<Artifact> {
    let started = std::time::Instant::now();
    // Opening a FIFO can block before any read or elapsed-time check. Follow
    // executable symlinks, but reject an existing special file before opening.
    validate_metadata(path, &std::fs::metadata(path)?, limit)?;
    let file = std::fs::File::open(path)?;
    let metadata = file.metadata()?;
    // The path preflight does not replace validation of the opened handle.
    validate_metadata(path, &metadata, limit)?;
    let mut file = file.take(limit + 1);
    let mut hash = Sha256::new();
    let mut bytes = 0;
    let mut buffer = [0; 65_536];
    loop {
        if started.elapsed() > std::time::Duration::from_secs(30) {
            bail!("Executable identity exceeded its 30-second budget");
        }
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
        bytes += count as u64;
    }
    if bytes != metadata.len() || bytes > limit {
        bail!(
            "Executable changed while its identity was collected: {}",
            path.display()
        );
    }
    Ok(Artifact {
        path: path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Executable path is not UTF-8"))?
            .to_owned(),
        digest: format!("sha256:{:x}", hash.finalize()),
        bytes,
    })
}

fn validate_metadata(path: &Path, metadata: &std::fs::Metadata, limit: u64) -> Result<()> {
    if !metadata.is_file() {
        bail!(
            "Executable identity requires a regular file: {}",
            path.display()
        );
    }
    if metadata.len() > limit {
        bail!(
            "Executable exceeds its {} MiB identity budget: {}",
            limit / (1024 * 1024),
            path.display()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regular_file_identity_preserves_exact_bytes_digest_and_size_limits() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("executable");
        let bytes = vec![0x81; 70_000];
        std::fs::write(&path, &bytes).unwrap();
        let artifact = identify(&path, bytes.len() as u64).unwrap();
        assert_eq!(artifact.path, path.to_str().unwrap());
        assert_eq!(artifact.bytes, bytes.len() as u64);
        assert_eq!(
            artifact.digest,
            "sha256:7da259f5bcf72dc549a4e361c4a24d5829e878f1758c62b05fea01063a866495"
        );
        assert!(
            identify(&path, bytes.len() as u64 - 1)
                .unwrap_err()
                .to_string()
                .contains("identity budget")
        );
        std::fs::write(&path, []).unwrap();
        assert_eq!(identify(&path, 0).unwrap().bytes, 0);
    }

    #[test]
    fn missing_files_and_directories_cannot_supply_executable_identity() {
        let root = tempfile::tempdir().unwrap();
        assert!(identify(&root.path().join("missing"), 10).is_err());
        assert!(
            identify(root.path(), 10)
                .unwrap_err()
                .to_string()
                .contains("regular file")
        );
    }

    #[cfg(unix)]
    #[test]
    fn executable_symlinks_follow_regular_files_but_reject_special_targets() {
        use std::os::unix::{fs::symlink, net::UnixListener};

        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("executable");
        let link = root.path().join("selected");
        std::fs::write(&path, b"executable bytes").unwrap();
        symlink(&path, &link).unwrap();
        let actual = identify(&link, 100).unwrap();
        assert_eq!(actual.path, link.to_str().unwrap());
        assert_eq!(actual.digest, identify(&path, 100).unwrap().digest);

        std::fs::remove_file(&link).unwrap();
        let socket = root.path().join("socket");
        let _listener = UnixListener::bind(&socket).unwrap();
        symlink(&socket, &link).unwrap();
        for candidate in [&socket, &link] {
            assert!(
                identify(candidate, 100)
                    .unwrap_err()
                    .to_string()
                    .contains("regular file")
            );
        }
    }
}
