//! Bounded process execution with whole-process-tree cancellation.

pub mod identity;

use anyhow::{Context, Result, bail};
use process_wrap::tokio::{ChildWrapper, CommandWrap, KillOnDrop};
use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};

pub const MAX_OUTPUT_BYTES: usize = 16 * 1024 * 1024;

struct ProcessGuard(Box<dyn ChildWrapper>);

impl Drop for ProcessGuard {
    fn drop(&mut self) {
        let _ = self.0.start_kill();
    }
}

#[derive(Debug)]
pub struct Output {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub exit_code: Option<i32>,
    pub started_at_ms: u64,
    pub ended_at_ms: u64,
    pub duration_ms: u64,
    pub timed_out: bool,
    pub capture_error: Option<String>,
}

/// Runs argv directly, without an implicit shell, and captures bounded streams.
/// Dropping this future kills the process group/job through the wrapper guard.
pub async fn capture(
    argv: &[String],
    cwd: &Path,
    input: Option<Vec<u8>>,
    deadline: Duration,
) -> Result<Output> {
    if argv.is_empty() || argv[0].is_empty() {
        bail!("Process argv must not be empty");
    }
    let start = std::time::Instant::now();
    let started_at_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_millis()
        .try_into()?;
    let mut command = CommandWrap::with_new(&argv[0], |command| {
        command
            .args(&argv[1..])
            .current_dir(cwd)
            .stdin(if input.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
    });
    command.wrap(KillOnDrop);
    #[cfg(unix)]
    command.wrap(process_wrap::tokio::ProcessGroup::leader());
    #[cfg(windows)]
    command.wrap(process_wrap::tokio::JobObject);
    let mut guard = ProcessGuard(
        command
            .spawn()
            .with_context(|| format!("Cannot start {}", argv[0]))?,
    );
    let child = &mut guard.0;
    let stdout = child.stdout().take().context("stdout was not piped")?;
    let stderr = child.stderr().take().context("stderr was not piped")?;
    let stdin = child.stdin().take();
    let write = async move {
        if let (Some(mut pipe), Some(input)) = (stdin, input) {
            pipe.write_all(&input).await?;
            pipe.shutdown().await?;
        }
        Ok::<_, anyhow::Error>(())
    };
    let mut stdout_bytes = Vec::new();
    let mut stderr_bytes = Vec::new();
    let work = async {
        let (status, _, _, _) = tokio::try_join!(
            async { child.wait().await.map_err(anyhow::Error::from) },
            read_bounded(stdout, &mut stdout_bytes),
            read_bounded(stderr, &mut stderr_bytes),
            write
        )?;
        Ok::<_, anyhow::Error>(status.code())
    };
    let result = tokio::time::timeout(deadline, work).await;
    // Kill descendants that outlive their direct parent, including on success.
    let _ = child.start_kill();
    let _ = tokio::time::timeout(Duration::from_secs(5), child.wait()).await;
    let (exit_code, timed_out, capture_error) = match result {
        Ok(Ok(code)) => (code, false, None),
        Ok(Err(error)) => (None, false, Some(error.to_string())),
        Err(_) => (None, true, None),
    };
    Ok(Output {
        stdout: stdout_bytes,
        stderr: stderr_bytes,
        exit_code,
        started_at_ms,
        ended_at_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_millis()
            .try_into()?,
        duration_ms: start.elapsed().as_millis().try_into()?,
        timed_out,
        capture_error,
    })
}

async fn read_bounded(reader: impl AsyncRead + Unpin, bytes: &mut Vec<u8>) -> Result<()> {
    reader
        .take((MAX_OUTPUT_BYTES + 1) as u64)
        .read_to_end(bytes)
        .await?;
    if bytes.len() > MAX_OUTPUT_BYTES {
        bail!("Process output exceeded {MAX_OUTPUT_BYTES} bytes");
    }
    Ok(())
}

#[cfg(test)]
#[path = "runner_tests.rs"]
mod tests;
