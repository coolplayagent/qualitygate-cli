//! Verifies recognized test commands when no structured test report is configured.

use crate::{
    config::{CommandCheck, ReportFormat},
    runner::Output,
};
use anyhow::{Context, Result, bail};
use std::{path::Path, sync::Arc};

#[derive(Clone, Copy)]
enum Source {
    None,
    Cargo,
    MissingReport,
}

/// Keep captured evidence available even if the parsing worker fails. The
/// worker borrows the original buffers through a shared owner, never a copy.
pub(crate) async fn from_capture(
    check: &CommandCheck,
    output: Output,
) -> (Arc<Output>, Result<Option<usize>>) {
    let source = source(check);
    let output = Arc::new(output);
    let counts = if matches!(source, Source::Cargo) {
        let captured = Arc::clone(&output);
        tokio::task::spawn_blocking(move || count(source, &captured.stdout))
            .await
            .context("Test-count parsing worker did not complete")
            .and_then(|result| result)
    } else {
        count(source, &output.stdout)
    };
    (output, counts)
}

fn source(check: &CommandCheck) -> Source {
    if check
        .reports
        .iter()
        .any(|report| report.format == ReportFormat::Junit || report.minimum_tests.is_some())
    {
        return Source::None;
    }
    let executable = check
        .argv
        .first()
        .and_then(|name| Path::new(name).file_stem())
        .and_then(|name| name.to_str())
        .unwrap_or("");
    if executable == "cargo" && check.argv.iter().any(|arg| arg == "test") {
        return Source::Cargo;
    }
    if ["pytest", "mvn", "mvnw", "gradle", "gradlew", "go"].contains(&executable)
        && (executable == "pytest"
            || check
                .argv
                .iter()
                .any(|arg| ["test", "verify"].contains(&arg.as_str())))
    {
        return Source::MissingReport;
    }
    Source::None
}

fn count(source: Source, bytes: &[u8]) -> Result<Option<usize>> {
    match source {
        Source::None => Ok(None),
        Source::MissingReport => {
            bail!("Test commands require a configured test-count report (for example JUnit)")
        }
        Source::Cargo => {
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
            Ok(Some(total))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command(argv: &str) -> CommandCheck {
        serde_norway::from_str(&format!("id: tests\nargv: {argv}\n")).unwrap()
    }

    fn from_output(check: &CommandCheck, bytes: &[u8]) -> Result<Option<usize>> {
        count(source(check), bytes)
    }

    #[tokio::test(flavor = "current_thread")]
    async fn counting_keeps_captured_buffers_and_metadata_when_parsing_fails() {
        for (argv, stdout, expected) in [
            (
                "[cargo, test]",
                "test result: FAILED. 2 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n",
                Some(Some(3)),
            ),
            ("[cargo, test]", "truncated test output", None),
            ("[cargo, build]", "build output", Some(None)),
            ("[pytest]", "test output without a report", None),
        ] {
            let captured = Output {
                stdout: stdout.as_bytes().to_vec(),
                stderr: b"captured stderr".to_vec(),
                exit_code: Some(101),
                started_at_ms: 100,
                ended_at_ms: 120,
                duration_ms: 20,
                timed_out: true,
                capture_error: Some("capture failure".into()),
            };
            let stdout_address = captured.stdout.as_ptr() as usize;
            let stderr_address = captured.stderr.as_ptr() as usize;
            let (captured, counts) = from_capture(&command(argv), captured).await;
            assert_eq!(counts.ok(), expected, "{argv}");
            assert_eq!(captured.stdout.as_ptr() as usize, stdout_address);
            assert_eq!(captured.stderr.as_ptr() as usize, stderr_address);
            assert_eq!(captured.stdout, stdout.as_bytes());
            assert_eq!(captured.stderr, b"captured stderr");
            assert_eq!(captured.exit_code, Some(101));
            assert_eq!(
                (
                    captured.started_at_ms,
                    captured.ended_at_ms,
                    captured.duration_ms
                ),
                (100, 120, 20)
            );
            assert!(captured.timed_out);
            assert_eq!(captured.capture_error.as_deref(), Some("capture failure"));
            assert_eq!(Arc::strong_count(&captured), 1);
        }
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
