//! Persist preflight details before presenting a bounded terminal summary.

use crate::{
    domain::{
        Artifact,
        doctor::{Diagnostic, Report},
    },
    paths, snapshot,
};
use anyhow::{Context, Result};
use std::io::{self, Write};

const MAX_REPORT_BYTES: usize = 16 * 1024 * 1024;

pub async fn persist(mut report: Report) -> Result<(Report, Option<Artifact>)> {
    tokio::task::spawn_blocking(move || {
        let artifact = match save(&report) {
            Ok(artifact) => Some(artifact),
            Err(error) => {
                report.complete = false;
                report.diagnostics.insert(
                    0,
                    Diagnostic::new(
                        "report.write_failed",
                        format!("Cannot save doctor report: {error:#}"),
                        Vec::new(),
                    ),
                );
                None
            }
        };
        (report, artifact)
    })
    .await
    .context("Doctor report persistence task failed")
}

fn save(report: &Report) -> Result<Artifact> {
    let mut bytes = BoundedBuffer(Vec::new());
    serde_json::to_writer_pretty(&mut bytes, report)?;
    let mut builder = tempfile::Builder::new();
    builder.prefix("qualitygate-doctor-");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        builder.permissions(std::fs::Permissions::from_mode(0o700));
    }
    let directory = builder.tempdir()?;
    let path = directory.path().canonicalize()?.join("report.json");
    // The private directory is retained only after a complete successful write.
    std::fs::write(&path, &bytes.0)?;
    let artifact = Artifact {
        path: paths::display(&path).to_string(),
        digest: snapshot::digest(&bytes.0),
        bytes: bytes.0.len() as u64,
    };
    let _ = directory.keep();
    Ok(artifact)
}

struct BoundedBuffer(Vec<u8>);

impl Write for BoundedBuffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_REPORT_BYTES.saturating_sub(self.0.len()) {
            return Err(io::Error::other("Doctor report exceeds 16 MiB"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_budget_rejects_overflow_without_partial_append() {
        let mut buffer = BoundedBuffer(Vec::new());
        buffer.write_all(&vec![b' '; MAX_REPORT_BYTES]).unwrap();
        buffer.flush().unwrap();
        assert!(buffer.write_all(b"x").is_err());
        assert_eq!(buffer.0.len(), MAX_REPORT_BYTES);
    }

    #[tokio::test]
    async fn oversized_report_is_incomplete_and_has_no_published_artifact() {
        let mut report = Report::new("full".into(), false);
        report.complete = true;
        report.diagnostics.push(Diagnostic::new(
            "example",
            "x".repeat(MAX_REPORT_BYTES),
            vec![],
        ));
        let (report, artifact) = persist(report).await.unwrap();
        assert_eq!(report.exit_code(), 2);
        assert!(!report.complete);
        assert!(artifact.is_none());
        assert_eq!(report.diagnostics[0].code, "report.write_failed");
        assert!(report.diagnostics[0].message.contains("16 MiB"));
    }
}
