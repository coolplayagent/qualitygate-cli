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
        let file = std::fs::File::open(&path)?;
        let metadata = file.metadata()?;
        if !metadata.is_file() || metadata.len() > MAX_EXECUTABLE_BYTES {
            bail!("Executable exceeds the 256 MiB identity budget: {program}");
        }
        let mut file = file.take(MAX_EXECUTABLE_BYTES + 1);
        let mut hash = Sha256::new();
        let mut bytes = 0;
        let mut buffer = [0; 65_536];
        loop {
            let count = file.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            hash.update(&buffer[..count]);
            bytes += count as u64;
        }
        if bytes != metadata.len() || bytes > MAX_EXECUTABLE_BYTES {
            bail!("Executable changed while its identity was collected: {program}");
        }
        Ok(Artifact {
            path: path
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("Executable path is not UTF-8"))?
                .to_owned(),
            digest: format!("sha256:{:x}", hash.finalize()),
            bytes,
        })
    })
    .await?
}
