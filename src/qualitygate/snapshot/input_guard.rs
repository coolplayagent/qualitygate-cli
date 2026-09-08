//! Detect changed execution inputs, including writes restored before validation.

use super::{File, content_digest};
use anyhow::{Context, Result, bail};
use std::{
    collections::BTreeMap,
    io::Read,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime},
};

#[derive(Debug, PartialEq, Eq)]
struct Stamp {
    modified: Option<SystemTime>,
    created: Option<SystemTime>,
    readonly: bool,
    #[cfg(unix)]
    unix: (u64, u64, i64, i64),
}

impl Stamp {
    fn from(metadata: &std::fs::Metadata) -> Self {
        Self {
            modified: metadata.modified().ok(),
            created: metadata.created().ok(),
            readonly: metadata.permissions().readonly(),
            #[cfg(unix)]
            unix: {
                use std::os::unix::fs::MetadataExt;
                (
                    metadata.dev(),
                    metadata.ino(),
                    metadata.ctime(),
                    metadata.ctime_nsec(),
                )
            },
        }
    }
}

struct Inputs {
    root: PathBuf,
    files: BTreeMap<String, File>,
    stamps: BTreeMap<String, Stamp>,
    invalid: Mutex<Option<String>>,
}

pub struct InputGuard {
    inputs: Arc<Inputs>,
    pub digest: String,
}

impl InputGuard {
    pub async fn new(root: &Path, files: BTreeMap<String, File>) -> Result<Self> {
        let root = root.to_owned();
        tokio::task::spawn_blocking(move || {
            let digest = content_digest(&files);
            let stamps = inspect(&root, &files)?;
            Ok(Self {
                inputs: Arc::new(Inputs {
                    root,
                    files,
                    stamps,
                    invalid: Mutex::new(None),
                }),
                digest,
            })
        })
        .await?
    }

    /// The first failure is permanent for this materialization. A later command
    /// cannot restore bytes to turn earlier invalid evidence into a valid result.
    pub async fn verify(&self) -> Result<()> {
        let inputs = Arc::clone(&self.inputs);
        tokio::task::spawn_blocking(move || {
            let mut invalid = inputs.invalid.lock().map_err(|_| anyhow::anyhow!("Input validation state unavailable"))?;
            if let Some(reason) = invalid.as_ref() {
                bail!("{reason}");
            }
            let check = (|| {
                let stamps = inspect(&inputs.root, &inputs.files)?;
                for (name, original) in &inputs.stamps {
                    if stamps.get(name) != Some(original) {
                        bail!("Checked input changed during execution, even if its bytes were restored: {name}");
                    }
                }
                Ok(())
            })();
            if let Err(error) = &check {
                *invalid = Some(format!("{error:#}"));
            }
            check
        }).await?
    }
}

fn inspect(root: &Path, files: &BTreeMap<String, File>) -> Result<BTreeMap<String, Stamp>> {
    let start = Instant::now();
    let mut stamps = BTreeMap::new();
    for (name, expected) in files {
        if start.elapsed() > Duration::from_secs(30) {
            bail!("Input validation exceeded its 30-second work budget");
        }
        let path = crate::paths::confined(root, Path::new(name)).with_context(|| {
            format!("Checked input removed or replaced during execution: {name}")
        })?;
        let metadata = std::fs::metadata(&path).with_context(|| {
            format!("Checked input removed or replaced during execution: {name}")
        })?;
        if !metadata.is_file() || metadata.len() != expected.bytes.len() as u64 {
            bail!("Checked input modified during execution: {name}");
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if (metadata.permissions().mode() & 0o111 != 0) != expected.executable {
                bail!("Checked input executable mode changed during execution: {name}");
            }
        }
        let before = Stamp::from(&metadata);
        let file = std::fs::File::open(&path)?;
        let mut bytes = Vec::new();
        file.take(expected.bytes.len() as u64 + 1)
            .read_to_end(&mut bytes)?;
        if bytes != expected.bytes {
            bail!("Checked input modified during execution: {name}");
        }
        let after = Stamp::from(&std::fs::metadata(&path)?);
        if before != after {
            bail!("Checked input changed while being validated: {name}");
        }
        stamps.insert(name.clone(), after);
    }
    Ok(stamps)
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn fixture() -> (tempfile::TempDir, InputGuard) {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("source.txt"), "initial\n").unwrap();
        let guard = InputGuard::new(
            root.path(),
            BTreeMap::from([(
                "source.txt".into(),
                File {
                    bytes: b"initial\n".to_vec(),
                    executable: false,
                },
            )]),
        )
        .await
        .unwrap();
        (root, guard)
    }

    #[tokio::test]
    async fn input_changes_remain_invalid_after_restoration() {
        let (root, guard) = fixture().await;
        guard.verify().await.unwrap();
        std::fs::write(root.path().join("source.txt"), "changed\n").unwrap();
        assert!(
            guard
                .verify()
                .await
                .unwrap_err()
                .to_string()
                .contains("modified")
        );
        std::fs::write(root.path().join("source.txt"), "initial\n").unwrap();
        assert!(guard.verify().await.is_err());
    }

    #[tokio::test]
    async fn replaced_or_deleted_inputs_cannot_reuse_the_original_evidence() {
        let (root, guard) = fixture().await;
        std::fs::remove_file(root.path().join("source.txt")).unwrap();
        std::fs::write(root.path().join("source.txt"), "initial\n").unwrap();
        assert!(
            guard
                .verify()
                .await
                .unwrap_err()
                .to_string()
                .contains("changed during execution")
        );
        let (root, guard) = fixture().await;
        std::fs::remove_file(root.path().join("source.txt")).unwrap();
        assert!(guard.verify().await.is_err());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn executable_bit_and_symlink_replacement_are_invalid() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let (root, guard) = fixture().await;
        std::fs::set_permissions(
            root.path().join("source.txt"),
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        assert!(
            guard
                .verify()
                .await
                .unwrap_err()
                .to_string()
                .contains("executable mode")
        );
        let (root, guard) = fixture().await;
        std::fs::rename(
            root.path().join("source.txt"),
            root.path().join("other.txt"),
        )
        .unwrap();
        symlink("other.txt", root.path().join("source.txt")).unwrap();
        assert!(guard.verify().await.is_err());
    }
}
