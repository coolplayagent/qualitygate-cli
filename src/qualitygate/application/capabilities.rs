//! No repository discovery, policy loading, network access, or producer execution.

use crate::{domain::runtime::*, runner};
use anyhow::Result;

pub async fn report() -> Result<Capabilities> {
    let executable = runner::identity::evaluator().await?;
    let (os, architecture) = crate::env::platform();
    Ok(Capabilities {
        schema_version: 1,
        cli_version: env!("CARGO_PKG_VERSION").into(),
        executable,
        platform: Platform {
            os: os.into(),
            architecture: architecture.into(),
        },
        capabilities: CAPABILITIES.iter().map(|id| (*id).into()).collect(),
        protocols: [
            ("capabilities", 1),
            ("doctor", 1),
            ("decision", 1),
            ("feedback", 1),
            ("configuration", 1),
            ("task", 1),
        ]
        .into_iter()
        .map(|(key, value)| (key.into(), value))
        .collect(),
        limits: Limits {
            snapshot_default_file_bytes: crate::domain::snapshot_budget::DEFAULT_FILE_BYTES,
            snapshot_max_file_bytes: crate::domain::snapshot_budget::MAX_FILE_BYTES,
            snapshot_default_bytes: crate::snapshot::MAX_SNAPSHOT_BYTES,
            snapshot_max_bytes: 1024 * 1024 * 1024,
            snapshot_max_files: crate::snapshot::MAX_FILES,
            snapshot_max_jobs: 16,
            snapshot_max_seconds: 3600,
            configuration_bytes: crate::config::MAX_CONFIG_BYTES,
            process_stream_bytes: runner::MAX_OUTPUT_BYTES,
            tool_version_bytes: 65_536,
            tool_probe_max_seconds: 60,
            doctor_probe_seconds: crate::domain::doctor::PROBE_SECONDS,
            diagnostic_details: crate::domain::doctor::DETAIL_LIMIT,
        },
    })
}
