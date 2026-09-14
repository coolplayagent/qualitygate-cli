//! Confined, content-addressed policy objects with one atomic index publication.

use crate::{domain::evolution::valid_digest, paths};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub const DIRECTORY: &str = ".qualitygate/policy";
#[cfg(test)]
#[path = "policy_store_tests.rs"]
mod tests;
pub const MAX_OBJECT_BYTES: usize = 8 * 1024 * 1024;
const MAX_ARCHIVE_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_OBJECTS: u64 = 100_000;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Index {
    pub schema_version: u32,
    pub sequence: u64,
    pub previous_state: Option<String>,
    pub last_event: Option<String>,
    pub active_policy: Option<String>,
    pub active_approval: Option<String>,
    pub active_authorization_kind: Option<String>,
    pub active_candidate: Option<String>,
    pub active_since: u64,
    pub policies: Vec<String>,
    pub evidence: Vec<String>,
    pub candidates: BTreeMap<String, String>,
    pub evaluations: Vec<String>,
    pub objects: u64,
    pub object_bytes: u64,
    /// Published non-index objects, including recovered objects from failed transactions.
    pub object_inventory: BTreeMap<String, u64>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record<T> {
    kind: String,
    value: T,
}

pub struct Store {
    root: PathBuf,
    pub index: Index,
    initial_head: Option<Vec<u8>>,
    deadline: Instant,
}

struct Lock {
    path: PathBuf,
    _file: std::fs::File,
}
impl Drop for Lock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

pub fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

pub fn now() -> Result<u64> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())
}

fn bounded(path: &Path) -> Result<Vec<u8>> {
    if !std::fs::symlink_metadata(path)?.is_file() {
        bail!("Policy objects must be regular non-symlink files");
    }
    let file = std::fs::File::open(path)?;
    if !file.metadata()?.is_file() {
        bail!("Policy object changed type while opening");
    }
    let mut bytes = Vec::new();
    file.take(MAX_OBJECT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_OBJECT_BYTES {
        bail!("Policy object exceeds 8 MiB");
    }
    Ok(bytes)
}

impl Store {
    /// An absent archive is a draft repository; corrupt or inaccessible archives are errors.
    pub fn optional(root: &Path) -> Result<Option<Self>> {
        let path = paths::confined(root, Path::new(&format!("{DIRECTORY}/HEAD")))?;
        match std::fs::symlink_metadata(path) {
            Ok(_) => Self::open(root).map(Some),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    pub fn open(root: &Path) -> Result<Self> {
        let root = dunce::canonicalize(root)?;
        let head = paths::confined(&root, Path::new(&format!("{DIRECTORY}/HEAD")))?;
        let bytes = bounded(&head)
            .context("No readable policy archive; retain evidence or create a candidate first")?;
        Self::from_head(root, Some(bytes))
    }

    fn from_head(root: PathBuf, head: Option<Vec<u8>>) -> Result<Self> {
        let mut store = Self {
            root,
            index: Index {
                schema_version: 1,
                ..Index::default()
            },
            initial_head: head,
            deadline: Instant::now() + Duration::from_secs(30),
        };
        if let Some(bytes) = &store.initial_head {
            let reference = std::str::from_utf8(bytes)?;
            store.index = store.record(reference, "index")?;
            if store.index.schema_version != 1
                || store.index.objects > MAX_OBJECTS
                || store.index.object_bytes > MAX_ARCHIVE_BYTES
                || store.index.sequence == 0
            {
                bail!("Invalid policy archive index version or resource counters");
            }
            if store.index.object_inventory.len() as u64 > store.index.objects
                || store
                    .index
                    .object_inventory
                    .iter()
                    .any(|(reference, bytes)| {
                        !valid_digest(reference) || *bytes > MAX_OBJECT_BYTES as u64
                    })
                || store.index.object_inventory.values().sum::<u64>() > store.index.object_bytes
            {
                bail!("Invalid policy object inventory or storage accounting");
            }
            let count = store.index.policies.len()
                + store.index.evidence.len()
                + store.index.candidates.len()
                + store.index.evaluations.len();
            if count as u64 > MAX_OBJECTS {
                bail!("Policy archive inventory exceeds object limit");
            }
            for reference in store
                .index
                .policies
                .iter()
                .chain(&store.index.evidence)
                .chain(store.index.candidates.values())
                .chain(&store.index.evaluations)
                .chain(store.index.last_event.iter())
                .chain(store.index.active_policy.iter())
                .chain(store.index.previous_state.iter())
            {
                if !valid_digest(reference) {
                    bail!("Invalid object reference in policy archive index");
                }
            }
        }
        Ok(store)
    }

    fn path(&self, name: &str) -> Result<PathBuf> {
        self.check_deadline()?;
        paths::confined(&self.root, Path::new(&format!("{DIRECTORY}/{name}")))
    }

    fn object_path(&self, reference: &str) -> Result<PathBuf> {
        if !valid_digest(reference) {
            bail!("Expected a lowercase sha256 content address");
        }
        self.path(&format!("objects/{}", &reference[7..]))
    }

    pub fn check_deadline(&self) -> Result<()> {
        if Instant::now() >= self.deadline {
            bail!("Policy archive operation exceeded its 30-second budget");
        }
        Ok(())
    }

    pub fn blob(&self, reference: &str) -> Result<Vec<u8>> {
        let bytes = bounded(&self.object_path(reference)?)?;
        if digest(&bytes) != reference {
            bail!("Policy object digest mismatch: {reference}");
        }
        self.check_deadline()?;
        Ok(bytes)
    }

    pub fn record<T: DeserializeOwned>(&self, reference: &str, kind: &str) -> Result<T> {
        let record: Record<T> = serde_json::from_slice(&self.blob(reference)?)?;
        if record.kind != kind {
            bail!("Policy object has kind {}, expected {kind}", record.kind);
        }
        Ok(record.value)
    }

    pub fn put_blob(&mut self, bytes: &[u8]) -> Result<String> {
        self.write_blob(bytes, true)
    }

    fn write_blob(&mut self, bytes: &[u8], account: bool) -> Result<String> {
        if bytes.len() > MAX_OBJECT_BYTES {
            bail!("Policy object exceeds 8 MiB");
        }
        let reference = digest(bytes);
        let path = self.object_path(&reference)?;
        let exists = match std::fs::symlink_metadata(&path) {
            Ok(_) => {
                self.blob(&reference)?;
                true
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
            Err(error) => return Err(error.into()),
        };
        let published = self.index.object_inventory.get(&reference);
        if published.is_some_and(|length| *length != bytes.len() as u64 || !exists) {
            bail!("Published policy object is missing or its inventory length differs");
        }
        let account = account && published.is_none();
        if account
            && (self.index.objects >= MAX_OBJECTS
                || self.index.object_bytes.saturating_add(bytes.len() as u64) > MAX_ARCHIVE_BYTES)
        {
            bail!(
                "Policy archive exceeds 100000 objects or 1 GiB; retain this archive and explicitly choose a new archive epoch"
            );
        }
        if !exists {
            let mut temporary = tempfile::NamedTempFile::new_in(
                path.parent().context("Object directory missing")?,
            )?;
            temporary.write_all(bytes)?;
            temporary.as_file().sync_all()?;
            self.check_deadline()?;
            temporary.persist_noclobber(&path)?;
        }
        if account {
            self.index.objects += 1;
            self.index.object_bytes += bytes.len() as u64;
            self.index
                .object_inventory
                .insert(reference.clone(), bytes.len() as u64);
        }
        Ok(reference)
    }

    pub fn put_record<T: Serialize>(&mut self, kind: &str, value: &T) -> Result<String> {
        self.put_blob(&encode(&Record {
            kind: kind.into(),
            value,
        })?)
    }

    pub fn event(&mut self, mut event: crate::domain::evolution::PolicyTransition) -> Result<()> {
        event.sequence = self.index.sequence + 1;
        event.previous = self.index.last_event.clone();
        self.index.last_event = Some(self.put_record("transition", &event)?);
        Ok(())
    }

    pub fn transaction<T>(
        root: &Path,
        operation: impl FnOnce(&mut Store) -> Result<T>,
    ) -> Result<T> {
        let root = dunce::canonicalize(root)?;
        let objects = paths::confined(&root, Path::new(&format!("{DIRECTORY}/objects")))?;
        std::fs::create_dir_all(&objects)?;
        let lock_path = paths::confined(&root, Path::new(&format!("{DIRECTORY}/write.lock")))?;
        let file = std::fs::OpenOptions::new().write(true).create_new(true).open(&lock_path)
            .context("Cannot acquire policy archive lock; another writer may be active. A crashed writer requires explicit lock recovery")?;
        let _lock = Lock {
            path: lock_path,
            _file: file,
        };
        let head_path = paths::confined(&root, Path::new(&format!("{DIRECTORY}/HEAD")))?;
        let head = match bounded(&head_path) {
            Ok(bytes) => Some(bytes),
            Err(error)
                if error
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|error| error.kind() == std::io::ErrorKind::NotFound) =>
            {
                None
            }
            Err(error) => return Err(error),
        };
        let mut store = Self::from_head(root, head)?;
        let before = serde_json::to_vec(&store.index)?;
        let result = operation(&mut store)?;
        if serde_json::to_vec(&store.index)? != before {
            store.commit(&head_path)?;
        }
        Ok(result)
    }

    fn commit(&mut self, head_path: &Path) -> Result<()> {
        self.index.sequence = self
            .index
            .sequence
            .checked_add(1)
            .context("Policy archive sequence overflow")?;
        self.index.previous_state = self
            .initial_head
            .as_ref()
            .map(|bytes| String::from_utf8(bytes.clone()))
            .transpose()?;
        // Account for this index object, including its own serialized counter.
        let previous_bytes = self.index.object_bytes;
        self.index.objects += 1;
        let mut bytes = serde_json::to_vec(&Record {
            kind: "index".into(),
            value: &self.index,
        })?;
        loop {
            self.index.object_bytes = previous_bytes + bytes.len() as u64;
            let encoded = serde_json::to_vec(&Record {
                kind: "index".into(),
                value: &self.index,
            })?;
            if encoded.len() == bytes.len() {
                bytes = encoded;
                break;
            }
            bytes = encoded;
        }
        if self.index.objects > MAX_OBJECTS || self.index.object_bytes > MAX_ARCHIVE_BYTES {
            bail!("Policy index would exceed the archive resource budget");
        }
        let reference = self.write_blob(&bytes, false)?;
        let mut temporary = tempfile::NamedTempFile::new_in(
            head_path.parent().context("Policy directory missing")?,
        )?;
        temporary.write_all(reference.as_bytes())?;
        temporary.as_file().sync_all()?;
        match (&self.initial_head, bounded(head_path)) {
            (Some(original), Ok(current)) if original == &current => {}
            (None, Err(error))
                if error
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|error| error.kind() == std::io::ErrorKind::NotFound) => {}
            _ => bail!("Policy archive HEAD changed during transaction"),
        }
        if self.path("HEAD")? != head_path {
            bail!("Policy archive path changed during transaction");
        }
        if self.initial_head.is_none() {
            temporary.persist_noclobber(head_path)?;
        } else {
            temporary.persist(head_path)?;
        }
        Ok(())
    }
}

/// Bound allocation during serialization, before retaining a large report.
fn encode(value: &impl Serialize) -> Result<Vec<u8>> {
    struct Buffer(Vec<u8>);
    impl Write for Buffer {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if self.0.len().saturating_add(bytes.len()) > MAX_OBJECT_BYTES {
                return Err(std::io::Error::other("Policy record exceeds 8 MiB"));
            }
            self.0.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut buffer = Buffer(Vec::new());
    serde_json::to_writer(&mut buffer, value)?;
    Ok(buffer.0)
}
