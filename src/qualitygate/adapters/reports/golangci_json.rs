//! Normalize golangci-lint v2 JSON without accepting analyzer errors as clean debt.

use super::{Data, Issue};
use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::collections::BTreeSet;

pub(super) fn parse(text: &str) -> Result<Data> {
    let root: Value = serde_json::from_str(text).context("Invalid golangci-lint JSON report")?;
    let report = root["Report"]
        .as_object()
        .context("golangci-lint report has no Report object")?;
    if report.get("Error").is_some_and(|error| {
        error
            .as_str()
            .is_none_or(|message| !message.trim().is_empty())
    }) {
        bail!("golangci-lint reported an analyzer error");
    }
    if let Some(warnings) = report.get("Warnings") {
        let warnings = warnings
            .as_array()
            .context("golangci-lint Warnings must be an array")?;
        if !warnings.is_empty() {
            bail!("golangci-lint reported analyzer warnings");
        }
    }
    let linters = report
        .get("Linters")
        .and_then(Value::as_array)
        .context("golangci-lint report has no Linters array")?;
    let mut names = BTreeSet::new();
    let mut enabled = BTreeSet::new();
    for linter in linters {
        let name = linter["Name"]
            .as_str()
            .filter(|name| !name.is_empty())
            .context("golangci-lint linter has no Name")?;
        if !names.insert(name.to_owned()) {
            bail!("golangci-lint report repeats a linter");
        }
        if linter
            .get("Enabled")
            .is_some_and(|value| !value.is_boolean())
        {
            bail!("golangci-lint Enabled must be a boolean");
        }
        if linter["Enabled"] == true {
            enabled.insert(name.to_owned());
        }
    }
    if enabled.is_empty() {
        bail!("golangci-lint report has no enabled linters");
    }
    let issues = root["Issues"]
        .as_array()
        .context("golangci-lint report has no Issues array")?;
    let mut data = Data::default();
    for issue in issues {
        let linter = issue["FromLinter"]
            .as_str()
            .filter(|linter| enabled.contains(*linter))
            .context("golangci-lint issue has no enabled linter")?;
        if linter == "typecheck" {
            bail!("golangci-lint reported a Go typecheck error");
        }
        let message = issue["Text"]
            .as_str()
            .filter(|message| !message.trim().is_empty())
            .context("golangci-lint issue has no message")?;
        let file = issue["Pos"]["Filename"]
            .as_str()
            .filter(|file| !file.is_empty())
            .context("golangci-lint issue has no source file")?;
        let line = issue["Pos"]["Line"]
            .as_u64()
            .and_then(|line| usize::try_from(line).ok())
            .filter(|line| *line > 0)
            .context("golangci-lint issue has no valid source line")?;
        let rule = message
            .split_once(':')
            .map(|(prefix, _)| prefix)
            .filter(|prefix| {
                prefix.len() <= 16
                    && prefix
                        .as_bytes()
                        .first()
                        .is_some_and(u8::is_ascii_alphabetic)
                    && prefix.bytes().all(|byte| byte.is_ascii_alphanumeric())
                    && prefix.bytes().any(|byte| byte.is_ascii_digit())
            })
            .unwrap_or(linter);
        data.issues.push(Issue {
            rule: rule.into(),
            file: Some(file.into()),
            line: Some(line),
            message: message.into(),
            symbol: None,
            tool: Some(linter.into()),
            locations: Vec::new(),
        });
    }
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::parse;

    const VALID: &str = r#"{"Issues":[{"FromLinter":"staticcheck","Text":"SA4006: value is never used","Pos":{"Filename":"src/main.go","Line":4}}],"Report":{"Linters":[{"Name":"staticcheck","Enabled":true},{"Name":"gosec"}]}}"#;

    #[test]
    fn parses_complete_linter_inventory_and_rejects_execution_errors() {
        let data = parse(VALID).unwrap();
        assert_eq!(data.issues[0].rule, "SA4006");
        assert_eq!(data.issues[0].tool.as_deref(), Some("staticcheck"));
        assert!(parse(&VALID.replace("\"Line\":4", "\"Line\":0")).is_err());
        assert!(parse(&VALID.replace("\"Enabled\":true", "\"Enabled\":false")).is_err());
        assert!(parse(&VALID.replace("\"staticcheck\"", "\"typecheck\"")).is_err());
        assert!(parse(&VALID.replace("\"Linters\"", "\"Missing\"")).is_err());
        assert!(
            parse(&VALID.replace("\"Report\":{", "\"Report\":{\"Error\":\"failed\",")).is_err()
        );
        assert!(parse(&VALID.replace("\"Report\":{", "\"Report\":{\"Warnings\":[{}],")).is_err());
        assert!(
            parse(r#"{"Issues":[],"Report":{"Linters":[{"Name":"staticcheck","Enabled":true}]}}"#)
                .is_ok()
        );
    }
}
