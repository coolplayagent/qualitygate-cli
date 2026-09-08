//! Environment access is centralized; credentials are never included in reports.

use std::path::PathBuf;

pub fn artifact_base() -> PathBuf {
    std::env::var_os("QUALITYGATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("qualitygate"))
}
