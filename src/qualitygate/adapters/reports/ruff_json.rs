//! Normalize Ruff's JSON findings while rejecting analysis failures as debt.

use super::{Data, Issue};
use anyhow::{Context, Result, bail};
use serde_json::Value;

pub(super) fn parse(text: &str) -> Result<Data> {
    let findings: Vec<Value> = serde_json::from_str(text).context("Invalid Ruff JSON report")?;
    let mut data = Data::default();
    for finding in findings {
        let code = finding["code"]
            .as_str()
            .filter(|code| !code.trim().is_empty())
            .context("Ruff finding has no rule code")?;
        if matches!(code, "invalid-syntax" | "E999" | "E902" | "io-error") {
            bail!("Ruff reported an analysis or source parsing failure");
        }
        let message = finding["message"]
            .as_str()
            .filter(|message| !message.trim().is_empty())
            .context("Ruff finding has no message")?;
        let file = finding["filename"]
            .as_str()
            .filter(|file| !file.trim().is_empty())
            .context("Ruff finding has no filename")?;
        let line = finding["location"]["row"]
            .as_u64()
            .and_then(|line| usize::try_from(line).ok())
            .filter(|line| *line > 0)
            .context("Ruff finding has no valid source row")?;
        let column = finding["location"]["column"]
            .as_u64()
            .filter(|column| *column > 0)
            .context("Ruff finding has no valid source column")?;
        let end_row = finding["end_location"]["row"]
            .as_u64()
            .context("Ruff finding has no valid end row")?;
        let end_column = finding["end_location"]["column"]
            .as_u64()
            .filter(|column| *column > 0)
            .context("Ruff finding has no valid end column")?;
        if end_row < line as u64 || (end_row == line as u64 && end_column < column) {
            bail!("Ruff finding has an inverted source range");
        }
        data.issues.push(Issue {
            rule: code.into(),
            file: Some(file.into()),
            line: Some(line),
            message: message.into(),
            symbol: None,
            tool: Some("ruff".into()),
            locations: Vec::new(),
        });
    }
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::parse;

    const VALID: &str = r#"[{"code":"F401","message":"unused import","filename":"src/app.py","location":{"row":2,"column":1},"end_location":{"row":2,"column":7}}]"#;

    #[test]
    fn parses_ruff_findings_and_rejects_incomplete_locations_or_analysis() {
        let data = parse(VALID).unwrap();
        assert_eq!(data.issues[0].rule, "F401");
        assert_eq!(data.issues[0].tool.as_deref(), Some("ruff"));
        assert!(parse("[]").is_ok());
        assert!(parse(&VALID.replace("\"F401\"", "\"invalid-syntax\"")).is_err());
        assert!(parse(&VALID.replace("\"row\":2", "\"row\":0")).is_err());
        assert!(parse(&VALID.replace("\"column\":1", "\"column\":0")).is_err());
        assert!(parse(&VALID.replace("\"filename\":\"src/app.py\"", "\"filename\":null")).is_err());
        assert!(parse("{\"code\":\"F401\"}").is_err());
    }
}
