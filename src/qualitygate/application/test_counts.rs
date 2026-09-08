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
        let pattern = regex::Regex::new(
            r"(?m)^test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; \d+ ignored; \d+ measured; \d+ filtered out; finished in [0-9.]+s\r?$",
        )?;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn command(argv: &str) -> CommandCheck {
        serde_norway::from_str(&format!("id: tests\nargv: {argv}\n")).unwrap()
    }

    #[test]
    fn rust_summaries_count_all_executed_suites_and_exclude_skips() {
        let check = command("[cargo, test]");
        let output = b"test result: ok. 2 passed; 0 failed; 9 ignored; 0 measured; 0 filtered out; finished in 0.01s\n\ntest result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s\n";
        assert_eq!(from_output(&check, output).unwrap(), Some(4));
        assert_eq!(from_output(&check, b"test result: ok. 0 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.00s\n").unwrap(), Some(0));
        assert!(from_output(&check, b"compiled, no tests run").is_err());
        assert!(from_output(&check, b"warning: test result: ok. 10 passed; 0 failed;\n").is_err());
        assert!(from_output(&check, b"test result: ok. 18446744073709551615 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n").is_err());
    }

    #[test]
    fn recognized_test_tools_require_reports_while_builds_use_exit_status() {
        for argv in [
            "[pytest]",
            "[./mvnw, test]",
            "[gradle, verify]",
            "[go, test, ./...]",
        ] {
            assert!(from_output(&command(argv), b"success").is_err(), "{argv}");
        }
        assert_eq!(from_output(&command("[cargo, build]"), b"").unwrap(), None);
        let mut check = command("[pytest]");
        check
            .reports
            .push(serde_norway::from_str("path: junit.xml\nformat: junit\n").unwrap());
        assert_eq!(from_output(&check, b"").unwrap(), None);
    }
}
