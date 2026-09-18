//! Normalize ESLint's built-in JSON formatter without treating configuration or parse errors as lint debt.

use super::{Data, Issue};
use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::collections::BTreeSet;

pub(super) fn parse(text: &str) -> Result<Data> {
    let files: Vec<Value> = serde_json::from_str(text).context("Invalid ESLint JSON report")?;
    if files.is_empty() {
        bail!("ESLint report has no analyzed files");
    }
    let mut data = Data::default();
    let mut seen = BTreeSet::new();
    for file in files {
        let path = file["filePath"]
            .as_str()
            .filter(|path| !path.is_empty())
            .context("ESLint result has no filePath")?;
        if !seen.insert(path.to_owned()) {
            bail!("ESLint report repeats a filePath");
        }
        let fatal = file["fatalErrorCount"]
            .as_u64()
            .context("ESLint result has no fatalErrorCount")?;
        if fatal != 0 {
            bail!("ESLint reported a fatal parse error");
        }
        let messages = file["messages"]
            .as_array()
            .context("ESLint result has no messages array")?;
        let mut errors = 0u64;
        let mut warnings = 0u64;
        for message in messages {
            let severity = message["severity"]
                .as_u64()
                .context("ESLint message has no severity")?;
            match severity {
                1 => warnings += 1,
                2 => errors += 1,
                _ => bail!("ESLint message severity must be 1 or 2"),
            }
            if message["fatal"] == true {
                bail!("ESLint reported a fatal parse error");
            }
            let rule = message["ruleId"]
                .as_str()
                .filter(|rule| !rule.is_empty())
                .context("ESLint message has no ruleId")?;
            let line = message["line"]
                .as_u64()
                .and_then(|line| usize::try_from(line).ok())
                .filter(|line| *line > 0)
                .context("ESLint message has no valid source line")?;
            let message = message["message"]
                .as_str()
                .filter(|message| !message.trim().is_empty())
                .context("ESLint message text is empty")?;
            data.issues.push(Issue {
                rule: rule.into(),
                file: Some(path.into()),
                line: Some(line),
                message: message.into(),
                symbol: None,
                tool: Some("eslint".into()),
                locations: Vec::new(),
            });
        }
        let reported_errors = file["errorCount"]
            .as_u64()
            .context("ESLint result has no errorCount")?;
        let reported_warnings = file["warningCount"]
            .as_u64()
            .context("ESLint result has no warningCount")?;
        if reported_errors != errors || reported_warnings != warnings {
            bail!("ESLint message counts contradict result summary");
        }
    }
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::parse;

    const VALID: &str = r#"[{"filePath":"src/app.js","errorCount":1,"warningCount":0,"fatalErrorCount":0,"messages":[{"ruleId":"no-eval","severity":2,"line":1,"message":"eval is unsafe"}]}]"#;

    #[test]
    fn parses_complete_eslint_results_and_rejects_fatal_or_inconsistent_reports() {
        let data = parse(VALID).unwrap();
        assert_eq!(data.issues.len(), 1);
        assert_eq!(data.issues[0].rule, "no-eval");
        assert!(parse("[]").is_err());
        assert!(parse(&VALID.replace("\"errorCount\":1", "\"errorCount\":0")).is_err());
        assert!(parse(&VALID.replace("\"fatalErrorCount\":0", "\"fatalErrorCount\":1")).is_err());
        assert!(parse(&VALID.replace("\"ruleId\":\"no-eval\"", "\"ruleId\":null")).is_err());
        assert!(parse(&VALID.replace("\"line\":1", "\"line\":0")).is_err());
        assert!(
            parse(&format!(
                "[{},{}]",
                &VALID[1..VALID.len() - 1],
                &VALID[1..VALID.len() - 1]
            ))
            .is_err()
        );
    }
}
