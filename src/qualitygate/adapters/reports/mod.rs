//! Normalizes real tool reports without treating missing or malformed data as pass.

mod coverage;
mod coverage_py;
mod jacoco;
mod json;
mod lcov;
mod sarif_locations;
mod xml;
mod xml_header;

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub locations: Vec<IssueLocation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IssueLocation {
    pub file: Option<String>,
    pub line: Option<usize>,
    pub end_line: Option<usize>,
    pub symbol: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SarifRun {
    pub tool: String,
    pub version: Option<String>,
    pub results: usize,
    pub violations: usize,
    pub non_violations: usize,
    pub suppressed_results: usize,
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
    #[serde(default)]
    pub excluded: bool,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub coverage_roots: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coverage_producer: Option<serde_json::Value>,
    #[serde(default)]
    pub branch_coverage: bool,
    #[serde(default)]
    pub affected_files: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sarif_runs: Vec<SarifRun>,
}

impl Data {
    pub fn has_findings(&self) -> bool {
        !self.issues.is_empty()
            || self.tests.as_ref().is_some_and(|tests| tests.failures > 0)
            || self.coverage.iter().any(|line| {
                !line.excluded && (line.hits == 0 || line.branches_hit < line.branches_found)
            })
    }
}

pub(super) fn coverage_budget(remaining: &mut usize, file: &str) -> Result<()> {
    if file.is_empty() || file.contains(['\0', '\r', '\n']) || file.len() > 16 * 1024 {
        bail!("Coverage source path is empty, unsafe or exceeds 16 KiB");
    }
    // JSON control-character escaping takes at most six bytes per path byte.
    *remaining = remaining
        .checked_sub(file.len() * 6 + 256)
        .ok_or_else(|| anyhow::anyhow!("Normalized coverage exceeds its 16 MiB budget"))?;
    Ok(())
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
        ReportFormat::CoveragePy => coverage_py::parse(text)?,
        ReportFormat::Diagnostics => serde_json::from_str(text)?,
        _ => xml::parse(format, text)?,
    };
    for issue in &data.issues {
        if issue.rule.is_empty() || issue.message.trim().is_empty() || issue.line == Some(0) {
            bail!("Report contains an invalid diagnostic");
        }
        if let Some(first) = issue.locations.first()
            && (first.file != issue.file
                || first.line != issue.line
                || first.symbol != issue.symbol)
        {
            bail!("Diagnostic primary location contradicts its location inventory");
        }
        for location in &issue.locations {
            if location.line == Some(0)
                || location
                    .end_line
                    .is_some_and(|end| location.line.is_none_or(|start| end < start))
            {
                bail!("Report contains an invalid diagnostic location range");
            }
        }
    }
    if data
        .tests
        .as_ref()
        .is_some_and(|tests| tests.failures > tests.executed)
    {
        bail!("Test failures exceed the executed test count");
    }
    let mut budget = 16 * 1024 * 1024;
    if data.coverage_roots.len() > 64 {
        bail!("Coverage source roots exceed 64 entries");
    }
    for file in data.coverage_files.iter().chain(&data.coverage_roots) {
        coverage_budget(&mut budget, file)?;
    }
    for line in &data.coverage {
        coverage_budget(&mut budget, &line.file)?;
        if line.file.is_empty()
            || line.line == 0
            || line.branches_hit > line.branches_found
            || (line.excluded && (line.hits != 0 || line.branches_found != 0))
        {
            bail!("Invalid coverage record");
        }
    }
    Ok(data)
}

#[cfg(test)]
#[path = "reports_tests.rs"]
mod tests;

#[cfg(test)]
mod coverage_tests;
