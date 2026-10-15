//! Export the already checked in-memory snapshot, never reopen the live worktree.
use super::Snapshot;
use crate::domain::{Artifact, delivery::Delivery};
use anyhow::{Result, ensure};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    path::Path,
    time::{Duration, Instant},
};

const MAX_ARCHIVE_BYTES: u64 = 2 * 1024 * 1024 * 1024;

struct HashingWriter<'a> {
    file: &'a std::fs::File,
    hash: Sha256,
    bytes: u64,
    started: Instant,
}
impl Write for HashingWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self.bytes.saturating_add(bytes.len() as u64) > MAX_ARCHIVE_BYTES
            || self.started.elapsed() > Duration::from_secs(120)
        {
            return Err(std::io::Error::other(
                "Delivery export exceeded its byte/time budget",
            ));
        }
        let count = self.file.write(bytes)?;
        self.hash.update(&bytes[..count]);
        self.bytes += count as u64;
        Ok(count)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}

/// The recipient must check the returned digest before consuming this object.
/// Local read-only permissions are not authentication or same-user isolation.
pub fn export(snapshot: &Snapshot, destination: &Path) -> Result<Delivery> {
    ensure!(
        snapshot.files.len() <= super::MAX_FILES
            && snapshot
                .files
                .values()
                .try_fold(0usize, |total, file| total.checked_add(file.bytes.len()))
                .is_some_and(|total| total <= 1024 * 1024 * 1024),
        "Delivery input exceeds the snapshot file/byte budget"
    );
    ensure!(
        super::content_digest(&snapshot.files) == snapshot.identity.content_digest,
        "Delivery inputs no longer match the frozen snapshot identity"
    );
    let destination = destination.canonicalize()?;
    ensure!(
        !destination.starts_with(&snapshot.root),
        "Delivery destination must be outside checked inputs"
    );
    let started = Instant::now();
    let temporary = tempfile::NamedTempFile::new_in(&destination)?;
    let artifact = {
        // Hash the exact stream generated from checked memory. Never derive the
        // expected digest from a potentially substituted filesystem object.
        let writer = HashingWriter {
            file: temporary.as_file(),
            hash: Sha256::new(),
            bytes: 0,
            started,
        };
        let mut zip = zip::ZipWriter::new_stream(writer);
        for (path, file) in &snapshot.files {
            ensure!(
                started.elapsed() < Duration::from_secs(120),
                "Delivery export exceeded 120 seconds"
            );
            ensure!(
                crate::paths::relative(Path::new(path))? == *path && !path.is_empty(),
                "Unsafe delivery path"
            );
            zip.start_file(
                path,
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Stored)
                    .unix_permissions(if file.executable { 0o755 } else { 0o644 }),
            )?;
            zip.write_all(&file.bytes)?;
            ensure!(
                temporary.as_file().metadata()?.len() <= MAX_ARCHIVE_BYTES,
                "Delivery archive exceeds 2 GiB"
            );
        }
        let writer = zip.finish()?.into_inner();
        Artifact {
            path: String::new(),
            bytes: writer.bytes,
            digest: format!("sha256:{:x}", writer.hash.finalize()),
        }
    };
    temporary.as_file().sync_all()?;
    let observed = identify(temporary.path())?;
    ensure!(
        observed.digest == artifact.digest && observed.bytes == artifact.bytes,
        "Delivery bytes changed before publication"
    );
    let name = format!(
        "snapshot-{}.zip",
        artifact.digest.trim_start_matches("sha256:")
    );
    let target = destination.join(name);
    match temporary.persist_noclobber(&target) {
        Ok(file) => {
            let mut permissions = file.metadata()?.permissions();
            permissions.set_readonly(true);
            file.set_permissions(permissions)?;
        }
        Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
            let existing = identify(&target)?;
            ensure!(
                existing.digest == artifact.digest && existing.bytes == artifact.bytes,
                "Existing delivery object was altered"
            );
        }
        Err(error) => return Err(error.into()),
    }
    Ok(Delivery {
        schema_version: 1,
        format: "qualitygate-snapshot-zip-v1".into(),
        artifact: Artifact {
            path: target
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("Delivery path must be UTF-8"))?
                .into(),
            ..artifact
        },
        snapshot: snapshot.identity.clone(),
        files: snapshot.files.len(),
    })
}

pub fn identify(path: &Path) -> Result<Artifact> {
    let started = Instant::now();
    let metadata = std::fs::symlink_metadata(path)?;
    ensure!(
        metadata.is_file() && metadata.len() <= MAX_ARCHIVE_BYTES,
        "Delivery object must be a bounded regular file"
    );
    let mut file = std::fs::File::open(path)?.take(MAX_ARCHIVE_BYTES + 1);
    let mut digest = Sha256::new();
    let mut bytes = 0u64;
    let mut buffer = [0u8; 65536];
    loop {
        ensure!(
            started.elapsed() < Duration::from_secs(120),
            "Delivery identity exceeded 120 seconds"
        );
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        bytes += count as u64;
        digest.update(&buffer[..count]);
    }
    ensure!(
        bytes == metadata.len() && bytes <= MAX_ARCHIVE_BYTES,
        "Delivery object changed while hashing"
    );
    Ok(Artifact {
        path: path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Delivery path must be UTF-8"))?
            .into(),
        bytes,
        digest: format!("sha256:{:x}", digest.finalize()),
    })
}
