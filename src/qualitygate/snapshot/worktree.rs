//! Bounded parallel filesystem reads with cooperative cancellation.

use super::{File, MAX_FILES, limits::Acquisition, run_git};
use anyhow::{Result, bail};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

struct Cancel(Arc<AtomicBool>);

impl Drop for Cancel {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

pub(super) async fn read(root: &Path, acquisition: &Acquisition) -> Result<BTreeMap<String, File>> {
    let listing = run_git(
        root,
        &[
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
        ],
        None,
    )
    .await?;
    let names = tokio::task::spawn_blocking(move || -> Result<_> {
        let names: BTreeSet<_> = listing
            .split(|byte| *byte == 0)
            .filter(|entry| !entry.is_empty())
            .map(|entry| String::from_utf8(entry.to_vec()))
            .collect::<Result<_, _>>()?;
        if names.len() > MAX_FILES {
            bail!("Snapshot exceeds {MAX_FILES} files");
        }
        Ok(names.into_iter().collect::<Vec<_>>())
    })
    .await??;
    let cancel = Cancel(Arc::new(AtomicBool::new(false)));
    let total = Arc::new(AtomicUsize::new(0));
    let mut pending = names.chunks(64);
    let mut tasks = tokio::task::JoinSet::new();
    let mut files = BTreeMap::new();
    loop {
        while tasks.len() < acquisition.options.jobs {
            let Some(names) = pending.next() else { break };
            let names = names.to_vec();
            let (root, total, stopped, permits, budget) = (
                root.to_owned(),
                total.clone(),
                cancel.0.clone(),
                acquisition.permits.clone(),
                acquisition.options.max_bytes,
            );
            let deadline = acquisition.deadline;
            let file_budget = acquisition.options.max_file_bytes;
            tasks.spawn(async move {
                let permit = permits.acquire_owned().await?;
                tokio::task::spawn_blocking(move || {
                    let _permit = permit;
                    let mut files = BTreeMap::new();
                    for name in names {
                        super::changes::check_deadline(Some(deadline))?;
                        if stopped.load(Ordering::Relaxed) {
                            bail!("Snapshot acquisition cancelled");
                        }
                        if let Some(file) = read_file(&root, &name, &total, budget, file_budget)? {
                            files.insert(name, file);
                        }
                    }
                    Ok::<_, anyhow::Error>(files)
                })
                .await?
            });
        }
        let Some(result) = tasks.join_next().await else {
            break;
        };
        files.extend(result??);
    }
    Ok(files)
}

fn read_file(
    root: &Path,
    name: &str,
    total: &AtomicUsize,
    budget: usize,
    file_budget: usize,
) -> Result<Option<File>> {
    let path = crate::paths::confined(root, Path::new(name))?;
    let metadata = match std::fs::metadata(&path) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if !metadata.is_file() {
        bail!("Unsupported snapshot entry (including submodule): {name}");
    }
    if metadata.len() > file_budget as u64 {
        bail!(crate::domain::snapshot_budget::file_limit_message(
            name,
            metadata.len(),
            file_budget
        ));
    }
    let size = metadata.len() as usize;
    if total.fetch_add(size, Ordering::Relaxed) + size > budget {
        bail!("Snapshot exceeds {budget} bytes at {name}; adjust --snapshot-max-mib");
    }
    let mut bytes = Vec::with_capacity(size);
    std::fs::File::open(&path)?
        .take((size + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() != size {
        bail!("Snapshot input changed while reading: {name}");
    }
    #[cfg(unix)]
    let executable = {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    };
    #[cfg(not(unix))]
    let executable = false;
    Ok(Some(File { bytes, executable }))
}
