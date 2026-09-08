//! Verifies recognized test commands when no structured test report is configured.

use crate::config::{CommandCheck, ReportFormat};
use anyhow::{Result, bail};
use std::path::Path;

pub(super) fn from_output(check: &CommandCheck, bytes: &[u8]) -> Result<Option<usize>> {
    if check
        .reports
        .iter()
        .any(|report| report.format == ReportFormat::Junit || report.minimum_tests.is_some())
    {
        return Ok(None);
    }
    let executable = check
        .argv
        .first()
        .and_then(|name| Path::new(name).file_stem())
        .and_then(|name| name.to_str())
        .unwrap_or("");
    if executable == "cargo" && check.argv.iter().any(|arg| arg == "test") {
        let text = std::str::from_utf8(bytes)?;
        let pattern =
            regex::Regex::new(r"test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed;")?;
        let mut total = 0usize;
        let mut matched = false;
        for counts in pattern.captures_iter(text) {
            matched = true;
            total = total
                .checked_add(counts[1].parse()?)
                .and_then(|value| value.checked_add(counts[2].parse().ok()?))
                .ok_or_else(|| anyhow::anyhow!("Test count overflow"))?;
        }
        if !matched {
            bail!(
                "Cargo test did not produce usable test counts; supply a structured test report for a custom runner"
            );
        }
        return Ok(Some(total));
    }
    if ["pytest", "mvn", "mvnw", "gradle", "gradlew", "go"].contains(&executable)
        && (executable == "pytest"
            || check
                .argv
                .iter()
                .any(|arg| ["test", "verify"].contains(&arg.as_str())))
    {
        bail!("Test commands require a configured test-count report (for example JUnit)");
    }
    Ok(None)
}
