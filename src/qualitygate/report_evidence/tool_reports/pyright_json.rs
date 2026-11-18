//! Read a completed Pyright analysis and retain PEP 702 diagnostics.

use super::{Data, Issue, IssueLocation, json_budget::Budget};
use anyhow::{Context, Result, ensure};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Report {
    version: String,
    time: String,
    general_diagnostics: Vec<Diagnostic>,
    summary: Summary,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Summary {
    files_analyzed: usize,
    error_count: usize,
    warning_count: usize,
    information_count: usize,
    time_in_sec: f64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Diagnostic {
    file: String,
    severity: Severity,
    message: String,
    range: Option<Range>,
    rule: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Severity {
    Error,
    Warning,
    Information,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Range {
    start: Position,
    end: Position,
}

#[derive(Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
struct Position {
    line: usize,
    character: usize,
}

pub(super) fn parse(text: &str) -> Result<Data> {
    let report: Report = serde_json::from_str(text).context("Invalid Pyright JSON report")?;
    semver::Version::parse(&report.version).context("Invalid Pyright version")?;
    ensure!(
        report.time.parse::<u64>().is_ok() && report.summary.time_in_sec >= 0.0,
        "Invalid Pyright completion time"
    );
    ensure!(
        report.summary.files_analyzed > 0,
        "Pyright did not analyze any files"
    );
    let mut actual = [0; 3];
    let mut data = Data::default();
    let mut budget = Budget::new("Normalized diagnostics");
    for diagnostic in report.general_diagnostics {
        actual[match diagnostic.severity {
            Severity::Error => 0,
            Severity::Warning => 1,
            Severity::Information => 2,
        }] += 1;
        ensure!(
            !diagnostic.message.trim().is_empty()
                && diagnostic
                    .rule
                    .as_ref()
                    .is_none_or(|rule| !rule.trim().is_empty())
                && diagnostic
                    .range
                    .as_ref()
                    .is_none_or(|range| range.start <= range.end),
            "Invalid Pyright diagnostic"
        );
        if diagnostic.rule.as_deref() == Some("reportDeprecated") {
            budget.push(&mut data.issues, deprecated(diagnostic)?)?;
        }
    }
    ensure!(
        actual
            == [
                report.summary.error_count,
                report.summary.warning_count,
                report.summary.information_count,
            ],
        "Pyright diagnostic counts contradict its completion summary"
    );
    ensure!(
        report.summary.error_count == 0,
        "Pyright reported analysis or type errors"
    );
    Ok(data)
}

fn deprecated(diagnostic: Diagnostic) -> Result<Issue> {
    ensure!(
        !diagnostic.file.trim().is_empty(),
        "Pyright deprecation finding has no file"
    );
    let range = diagnostic
        .range
        .context("Pyright deprecation finding has no source range")?;
    let line = range
        .start
        .line
        .checked_add(1)
        .context("Pyright source line overflow")?;
    // A range ending at column zero on a later line excludes that line.
    let end_line = range
        .end
        .line
        .checked_add(usize::from(
            range.end.character > 0 || range.end.line == range.start.line,
        ))
        .context("Pyright source end line overflow")?;
    Ok(Issue {
        rule: "reportDeprecated".into(),
        file: Some(diagnostic.file.clone()),
        line: Some(line),
        message: diagnostic.message,
        symbol: None,
        tool: Some("pyright".into()),
        locations: vec![IssueLocation {
            file: Some(diagnostic.file),
            line: Some(line),
            end_line: Some(end_line),
            symbol: None,
        }],
    })
}

#[cfg(test)]
mod tests {
    use super::parse;
    use serde_json::{Value, json};

    fn complete() -> Value {
        json!({
            "version": "1.1.414",
            "time": "1794993000123",
            "generalDiagnostics": [],
            "summary": {
                "filesAnalyzed": 2,
                "errorCount": 0,
                "warningCount": 0,
                "informationCount": 0,
                "timeInSec": 0.561,
            }
        })
    }

    fn deprecated() -> Value {
        json!({
            "file": "src/app.py",
            "severity": "warning",
            "message": "The function \"old\" is deprecated",
            "range": {"start": {"line": 0, "character": 1}, "end": {"line": 2, "character": 0}},
            "rule": "reportDeprecated",
        })
    }

    #[test]
    fn retains_only_deprecations_with_one_based_inclusive_lines() {
        let mut report = complete();
        assert!(parse(&report.to_string()).unwrap().issues.is_empty());
        report["generalDiagnostics"] = json!([
            deprecated(),
            {"file":"", "severity":"information", "message":"unrelated informational message"},
        ]);
        report["summary"]["warningCount"] = json!(1);
        report["summary"]["informationCount"] = json!(1);
        let data = parse(&report.to_string()).unwrap();
        assert_eq!(data.issues.len(), 1);
        let issue = &data.issues[0];
        assert_eq!(issue.rule, "reportDeprecated");
        assert_eq!(issue.tool.as_deref(), Some("pyright"));
        assert_eq!(issue.file.as_deref(), Some("src/app.py"));
        assert_eq!(issue.line, Some(1));
        assert_eq!(issue.locations[0].end_line, Some(2));
    }

    #[test]
    fn rejects_incomplete_or_inconsistent_analysis() {
        let mut report = complete();
        for (key, value) in [
            ("filesAnalyzed", json!(0)),
            ("timeInSec", json!(-1)),
            ("warningCount", json!(1)),
        ] {
            let previous = report["summary"][key].take();
            report["summary"][key] = value;
            assert!(parse(&report.to_string()).is_err());
            report["summary"][key] = previous;
        }
        report["generalDiagnostics"] = json!([
            {"file": "src/app.py", "severity":"error", "message":"Type error", "rule":"reportArgumentType"},
        ]);
        report["summary"]["errorCount"] = json!(1);
        assert!(parse(&report.to_string()).is_err());
        assert!(parse("{}").is_err());
        assert!(parse(&complete().to_string().replace("1.1.414", "unknown")).is_err());
        assert!(parse(&complete().to_string().replace("1794993000123", "invalid")).is_err());
    }

    #[test]
    fn rejects_invalid_deprecation_locations() {
        let mut report = complete();
        report["summary"]["warningCount"] = json!(1);
        for (key, value) in [
            ("file", json!("")),
            ("range", Value::Null),
            (
                "range",
                json!({"start": {"line": 1, "character": 3}, "end": {"line": 1, "character": 2}}),
            ),
            (
                "range",
                json!({"start": {"line": usize::MAX, "character": 0}, "end": {"line": usize::MAX, "character": 1}}),
            ),
            (
                "range",
                json!({"start": {"line": 0, "character": 0}, "end": {"line": usize::MAX, "character": 1}}),
            ),
        ] {
            let mut finding = deprecated();
            finding[key] = value;
            report["generalDiagnostics"] = json!([finding]);
            assert!(parse(&report.to_string()).is_err());
        }
    }
}
