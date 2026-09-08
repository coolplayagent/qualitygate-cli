//! Normalizes real tool reports without treating missing or malformed data as pass.

mod coverage;
mod json;
mod lcov;
mod xml;

use crate::config::ReportFormat;
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Issue {
    pub rule: String,
    pub file: Option<String>,
    pub line: Option<usize>,
    pub message: String,
    #[serde(default)]
    pub symbol: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tests {
    pub executed: usize,
    pub failures: usize,
    pub skipped: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoverageLine {
    pub file: String,
    pub line: usize,
    pub hits: u64,
    pub branches_found: usize,
    pub branches_hit: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Data {
    pub issues: Vec<Issue>,
    #[serde(default)]
    pub tests: Option<Tests>,
    #[serde(default)]
    pub coverage: Vec<CoverageLine>,
    #[serde(default)]
    pub coverage_files: Vec<String>,
    #[serde(default)]
    pub branch_coverage: bool,
    #[serde(default)]
    pub affected_files: Option<Vec<String>>,
}

pub fn parse(format: ReportFormat, bytes: &[u8]) -> Result<Data> {
    if bytes.len() > crate::snapshot::MAX_FILE_BYTES {
        bail!("Report exceeds size budget");
    }
    let text = std::str::from_utf8(bytes)?;
    if text.trim().is_empty() {
        bail!("Report is empty");
    }
    let data = match format {
        ReportFormat::Lcov => lcov::parse(text)?,
        ReportFormat::Sarif => json::sarif(text)?,
        ReportFormat::Diagnostics => serde_json::from_str(text)?,
        _ => xml::parse(format, text)?,
    };
    for issue in &data.issues {
        if issue.rule.is_empty() || issue.message.trim().is_empty() || issue.line == Some(0) {
            bail!("Report contains an invalid diagnostic");
        }
    }
    if data
        .tests
        .as_ref()
        .is_some_and(|tests| tests.failures > tests.executed)
    {
        bail!("Test failures exceed the executed test count");
    }
    for line in &data.coverage {
        if line.file.is_empty() || line.line == 0 || line.branches_hit > line.branches_found {
            bail!("Invalid coverage record");
        }
    }
    Ok(data)
}

#[cfg(test)]
#[path = "reports_tests.rs"]
mod tests;
