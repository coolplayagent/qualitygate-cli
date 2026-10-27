//! Protect execution inputs and detect changes, including restored writes.

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
    #[cfg(windows)]
    file_id: file_id::FileId,
}

impl Stamp {
    fn from(metadata: &std::fs::Metadata, _path: &Path) -> Result<Self> {
        Ok(Self {
            modified: metadata.modified().ok(),
            created: metadata.created().ok(),
            readonly: metadata.permissions().readonly(),
            #[cfg(windows)]
            file_id: file_id::get_file_id(_path)
                .context("Cannot establish Windows input file identity")?,
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
        })
    }
}

struct Inputs {
    root: PathBuf,
    files: ExpectedFiles,
    stamps: BTreeMap<String, Stamp>,
    invalid: Mutex<Option<String>>,
    #[cfg(windows)]
    _read_handles: Vec<std::fs::File>,
}

enum ExpectedFiles {
    Owned(BTreeMap<String, File>),
    Snapshot(Arc<super::Snapshot>),
}
impl ExpectedFiles {
    fn files(&self) -> &BTreeMap<String, File> {
        match self {
            Self::Owned(files) => files,
            Self::Snapshot(snapshot) => &snapshot.files,
        }
    }

    fn digest(&self) -> String {
        match self {
            Self::Owned(files) => content_digest(files),
            Self::Snapshot(snapshot) => snapshot.identity.content_digest.clone(),
        }
    }
}

pub struct InputGuard {
    inputs: Arc<Inputs>,
    pub digest: String,
}

impl InputGuard {
    pub async fn new(root: &Path, files: BTreeMap<String, File>) -> Result<Self> {
        Self::create(root, ExpectedFiles::Owned(files)).await
    }

    pub async fn for_snapshot(root: &Path, snapshot: Arc<super::Snapshot>) -> Result<Self> {
        Self::create(root, ExpectedFiles::Snapshot(snapshot)).await
    }

    async fn create(root: &Path, files: ExpectedFiles) -> Result<Self> {
        let root = root.to_owned();
        tokio::task::spawn_blocking(move || {
            let deadline = Instant::now() + Duration::from_secs(30);
            let digest = files.digest();
            #[cfg(windows)]
            let read_handles = protect_windows_inputs(&root, files.files(), deadline)?;
            let stamps = inspect(&root, files.files(), deadline)?;
            Ok(Self {
                inputs: Arc::new(Inputs {
                    root,
                    files,
                    stamps,
                    invalid: Mutex::new(None),
                    #[cfg(windows)]
                    _read_handles: read_handles,
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
                let stamps = inspect(&inputs.root, inputs.files.files(), Instant::now() + Duration::from_secs(30))?;
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

#[cfg(windows)]
fn protect_windows_inputs(
    root: &Path,
    files: &BTreeMap<String, File>,
    deadline: Instant,
) -> Result<Vec<std::fs::File>> {
    use std::os::windows::fs::OpenOptionsExt;

    // FILE_SHARE_READ permits readers while denying data writers and deletion
    // until these handles close. Windows modification times can be restored,
    // and the stable Metadata API does not expose its change timestamp.
    const FILE_SHARE_READ: u32 = 0x0000_0001;
    super::io_workers::map(
        files,
        deadline,
        "Windows input protection exceeded its 30-second work budget",
        |name, _| {
            let path = crate::paths::confined(root, Path::new(name))?;
            if !std::fs::metadata(&path)?.is_file() {
                bail!("Checked input is not a regular file: {name}");
            }
            let file = std::fs::OpenOptions::new()
                .read(true)
                .share_mode(FILE_SHARE_READ)
                .open(&path)
                .with_context(|| format!("Cannot protect Windows checked input: {name}"))?;
            if !file.metadata()?.is_file() {
                bail!("Checked input is not a regular file: {name}");
            }
            Ok(file)
        },
    )
}

fn inspect(
    root: &Path,
    files: &BTreeMap<String, File>,
    deadline: Instant,
) -> Result<BTreeMap<String, Stamp>> {
    let stamps = super::io_workers::map(
        files,
        deadline,
        "Input validation exceeded its 30-second work budget",
        |name, expected| {
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
            let before = Stamp::from(&metadata, &path)?;
            let file = std::fs::File::open(&path)?;
            let mut bytes = Vec::new();
            file.take(expected.bytes.len() as u64 + 1)
                .read_to_end(&mut bytes)?;
            if bytes != expected.bytes {
                bail!("Checked input modified during execution: {name}");
            }
            let after = Stamp::from(&std::fs::metadata(&path)?, &path)?;
            if before != after {
                bail!("Checked input changed while being validated: {name}");
            }
            Ok((name.to_owned(), after))
        },
    )?;
    Ok(stamps.into_iter().collect())
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
    async fn data_writes_are_prevented_or_permanently_invalidate_inputs() {
        let (root, guard) = fixture().await;
        guard.verify().await.unwrap();
        let path = root.path().join("source.txt");
        let write = std::fs::write(&path, "changed\n");
        #[cfg(windows)]
        {
            assert_sharing_violation(write);
            assert_eq!(std::fs::read(&path).unwrap(), b"initial\n");
            guard.verify().await.unwrap();
        }
        #[cfg(not(windows))]
        {
            write.unwrap();
            assert!(
                guard
                    .verify()
                    .await
                    .unwrap_err()
                    .to_string()
                    .contains("modified")
            );
            std::fs::write(&path, "initial\n").unwrap();
            assert!(guard.verify().await.is_err());
        }
    }

    #[tokio::test]
    async fn attribute_changes_remain_invalid_after_restoration() {
        let (root, guard) = fixture().await;
        let path = root.path().join("source.txt");
        let permissions = std::fs::metadata(&path).unwrap().permissions();
        let mut changed = permissions.clone();
        changed.set_readonly(true);
        std::fs::set_permissions(&path, changed).unwrap();
        let invalid = guard.verify().await;
        std::fs::set_permissions(&path, permissions).unwrap();
        assert!(invalid.is_err());
        assert!(guard.verify().await.is_err());
    }

    #[tokio::test]
    async fn replaced_or_deleted_inputs_cannot_reuse_the_original_evidence() {
        let (root, guard) = fixture().await;
        let path = root.path().join("source.txt");
        #[cfg(windows)]
        {
            assert_sharing_violation(std::fs::remove_file(&path));
            assert_sharing_violation(std::fs::rename(&path, root.path().join("replaced.txt")));
            guard.verify().await.unwrap();
            drop(guard);
            std::fs::remove_file(&path).unwrap();
        }
        #[cfg(not(windows))]
        {
            std::fs::remove_file(&path).unwrap();
            std::fs::write(&path, "initial\n").unwrap();
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
    }

    #[test]
    fn preserving_contents_and_modification_time_does_not_preserve_file_identity() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("source.txt");
        std::fs::write(&path, "initial\n").unwrap();
        let metadata = std::fs::metadata(&path).unwrap();
        let original = Stamp::from(&metadata, &path).unwrap();
        let modified = metadata.modified().unwrap();
        std::fs::rename(&path, root.path().join("original.txt")).unwrap();
        std::fs::write(&path, "initial\n").unwrap();
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(modified))
            .unwrap();
        let replacement = Stamp::from(&std::fs::metadata(&path).unwrap(), &path).unwrap();
        assert_eq!(replacement.modified, original.modified);
        #[cfg(windows)]
        assert_ne!(replacement.file_id, original.file_id);
        assert_ne!(replacement, original);
    }

    #[tokio::test]
    async fn in_place_writes_cannot_be_hidden_by_restoring_contents_and_modified_time() {
        use std::io::{Seek, SeekFrom, Write};

        let (root, guard) = fixture().await;
        let path = root.path().join("source.txt");
        let original = std::fs::metadata(&path).unwrap();
        let writer = std::fs::File::options().write(true).open(&path);
        #[cfg(windows)]
        if let Err(error) = &writer {
            assert_eq!(error.raw_os_error(), Some(32), "{error}");
            guard.verify().await.unwrap();
            assert_eq!(std::fs::read(&path).unwrap(), b"initial\n");
            return;
        }
        {
            let mut file = writer.unwrap();
            file.write_all(b"changed\n").unwrap();
            file.seek(SeekFrom::Start(0)).unwrap();
            file.write_all(b"initial\n").unwrap();
            file.flush().unwrap();
        }
        // Close the writer before restoring its timestamp, and close that
        // handle too before observing metadata or validating the guard.
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(original.modified().unwrap()))
            .unwrap();
        let restored = std::fs::metadata(&path).unwrap();
        assert_eq!(restored.modified().unwrap(), original.modified().unwrap());
        assert_eq!(std::fs::read(&path).unwrap(), b"initial\n");
        assert!(
            guard.verify().await.is_err(),
            "in-place writes with restored bytes and modification time reused input evidence"
        );
    }

    #[cfg(windows)]
    fn assert_sharing_violation<T>(result: std::io::Result<T>) {
        let error = result
            .err()
            .expect("checked input operation must be prevented");
        assert_eq!(error.raw_os_error(), Some(32), "{error}");
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn windows_handles_allow_readers_and_release_after_drop_or_construction_failure() {
        let (root, guard) = fixture().await;
        let path = root.path().join("source.txt");
        assert_eq!(std::fs::read(&path).unwrap(), b"initial\n");
        let second = InputGuard::new(root.path(), guard.inputs.files.files().clone())
            .await
            .unwrap();
        drop(guard);
        assert_sharing_violation(std::fs::write(&path, "changed\n"));
        drop(second);
        std::fs::write(&path, "changed\n").unwrap();

        let first = root.path().join("a.txt");
        let last = root.path().join("z.txt");
        std::fs::write(&first, "first\n").unwrap();
        std::fs::write(&last, "last\n").unwrap();
        let files = BTreeMap::from([
            (
                "a.txt".into(),
                File {
                    bytes: b"first\n".to_vec(),
                    executable: false,
                },
            ),
            (
                "z.txt".into(),
                File {
                    bytes: b"last\n".to_vec(),
                    executable: false,
                },
            ),
        ]);
        let writer = std::fs::File::options().write(true).open(&last).unwrap();
        assert!(
            InputGuard::new(root.path(), files.clone())
                .await
                .err()
                .unwrap()
                .to_string()
                .contains("Cannot protect Windows checked input")
        );
        std::fs::write(&first, "first\n").unwrap();
        drop(writer);
        std::fs::write(&last, "different\n").unwrap();
        assert!(InputGuard::new(root.path(), files).await.is_err());
        std::fs::write(&first, "restored\n").unwrap();
        std::fs::write(&last, "restored\n").unwrap();
        root.close().unwrap();
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
