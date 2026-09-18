//! Cargo's JSON Lines compiler messages from `cargo clippy --message-format=json`.

use super::{Data, Issue};
use anyhow::{Context, Result, bail};
use serde_json::Value;

pub(super) fn parse(text: &str) -> Result<Data> {
    let mut data = Data::default();
    let mut finished = None;
    let mut compiler_errors = false;
    let mut denied_lints = 0usize;
    for line in text.lines() {
        if line.trim().is_empty() {
            bail!("Cargo Clippy output contains an empty record");
        }
        let record: Value = serde_json::from_str(line).context("Invalid Cargo JSON record")?;
        let reason = record["reason"]
            .as_str()
            .context("Cargo JSON record has no reason")?;
        if finished.is_some() {
            bail!("Cargo Clippy output continues after build-finished");
        }
        match reason {
            "build-finished" => {
                finished = Some(
                    record["success"]
                        .as_bool()
                        .context("Cargo build-finished has no success value")?,
                );
            }
            "compiler-message" => {
                let message = &record["message"];
                let level = message["level"]
                    .as_str()
                    .context("Cargo compiler message has no level")?;
                if !["warning", "error", "note", "help", "failure-note"].contains(&level) {
                    bail!("Unknown Cargo compiler message level: {level}");
                }
                if !["warning", "error"].contains(&level) {
                    continue;
                }
                let code = message["code"]["code"].as_str();
                if level == "error" && code.is_none_or(|code| code.starts_with('E')) {
                    compiler_errors = true;
                    continue;
                }
                let rule = code.context("Cargo lint message has no lint code")?;
                if rule.is_empty() {
                    bail!("Cargo lint code is empty");
                }
                if ["unknown_lints", "renamed_and_removed_lints"].contains(&rule) {
                    bail!("Cargo Clippy reported an invalid lint selection: {rule}");
                }
                if level == "error" {
                    denied_lints += 1;
                }
                let spans = message["spans"]
                    .as_array()
                    .context("Cargo lint message has no spans")?;
                let primary = spans
                    .iter()
                    .find(|span| span["is_primary"] == true)
                    .context("Cargo lint message has no primary span")?;
                let file = primary["file_name"]
                    .as_str()
                    .context("Cargo primary span has no source path")?;
                let line = primary["line_start"]
                    .as_u64()
                    .and_then(|line| usize::try_from(line).ok())
                    .filter(|line| *line > 0)
                    .context("Cargo primary span has no valid line")?;
                let message = message["message"]
                    .as_str()
                    .filter(|value| !value.trim().is_empty())
                    .context("Cargo lint message is empty")?;
                data.issues.push(Issue {
                    rule: rule.into(),
                    file: Some(file.into()),
                    line: Some(line),
                    message: message.into(),
                    symbol: None,
                    tool: Some("clippy".into()),
                    locations: Vec::new(),
                });
            }
            "compiler-artifact" | "build-script-executed" | "text-line" => {}
            other => bail!("Unsupported Cargo JSON record: {other}"),
        }
    }
    let success = finished.context("Cargo Clippy output lacks build-finished")?;
    if compiler_errors || (!success && denied_lints == 0) || (success && denied_lints > 0) {
        bail!("Cargo Clippy compilation failed; lint evidence is incomplete");
    }
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::parse;

    const LINT: &str = r#"{"reason":"compiler-message","message":{"level":"warning","code":{"code":"clippy::len_zero"},"message":"use is_empty","spans":[{"is_primary":true,"file_name":"src/lib.rs","line_start":1}]}}"#;

    #[test]
    fn parses_complete_lint_stream_and_rejects_missing_or_failed_build_evidence() {
        let report = format!("{LINT}\n{{\"reason\":\"build-finished\",\"success\":true}}\n");
        let data = parse(&report).unwrap();
        assert_eq!(data.issues.len(), 1);
        assert_eq!(data.issues[0].rule, "clippy::len_zero");
        assert!(parse(LINT).is_err());
        assert!(parse("{\"reason\":\"build-finished\",\"success\":false}").is_err());
        assert!(parse(&format!("{report}not json\n")).is_err());
        assert!(
            parse(&format!(
                "{LINT}\n{{\"reason\":\"build-finished\",\"success\":false}}"
            ))
            .is_err()
        );
        let denied = LINT.replace("\"warning\"", "\"error\"");
        assert_eq!(
            parse(&format!(
                "{denied}\n{{\"reason\":\"build-finished\",\"success\":false}}"
            ))
            .unwrap()
            .issues
            .len(),
            1
        );
        assert!(
            parse(&format!(
                "{denied}\n{{\"reason\":\"build-finished\",\"success\":true}}"
            ))
            .is_err()
        );
        assert!(parse(&report.replace("clippy::len_zero", "unknown_lints")).is_err());
        let error = r#"{"reason":"compiler-message","message":{"level":"error","code":{"code":"E0425"},"message":"unknown name","spans":[]}}"#;
        assert!(
            parse(&format!(
                "{error}\n{{\"reason\":\"build-finished\",\"success\":false}}"
            ))
            .is_err()
        );
    }
}
