//! OpenJDK's detailed English deprecation and removal warnings.

use super::{Data, Issue, json_budget::Budget};
use anyhow::{Context, Result, ensure};

pub(super) fn parse(text: &str) -> Result<Data> {
    // A successful javac invocation creates an empty -Xstdout file when clean.
    // The command check also requires the fresh file and the expected exit.
    ensure!(
        text.is_empty() || !text.trim().is_empty() && text.ends_with('\n'),
        "javac report is blank or truncated"
    );
    let mut data = Data::default();
    let mut budget = Budget::new("Normalized diagnostics");
    let mut warnings = 0usize;
    let mut summary = None;

    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        ensure!(
            summary.is_none(),
            "Unexpected output after the javac warning summary"
        );
        if line.trim().is_empty() {
            continue;
        }
        if let Some((count, "warning" | "warnings")) = line.split_once(' ') {
            summary = Some(
                count
                    .parse::<usize>()
                    .context("Invalid javac warning count")?,
            );
            continue;
        }
        if let Some(note) = line.strip_prefix("Note: ") {
            ensure!(
                !note.contains("deprecated API")
                    && !note.contains("-Xlint:deprecation")
                    && !note.contains("-Xlint:removal"),
                "javac reported unexpanded deprecation diagnostics; enable -Xlint:deprecation,removal"
            );
            continue;
        }
        if let Some(message) = line.strip_prefix("warning: ") {
            ensure!(
                lint(message)?.is_none(),
                "javac deprecation or removal warning lacks a source location"
            );
            warnings += 1;
            continue;
        }
        let (position, message) = line
            .rsplit_once(": warning: ")
            .context("Unexpected or incomplete javac diagnostic output")?;
        let (file, number) = position
            .rsplit_once(':')
            .context("javac warning lacks a source line")?;
        let number = number
            .parse::<usize>()
            .context("Invalid javac source line")?;
        ensure!(
            number > 0 && file.ends_with(".java"),
            "Invalid javac source location"
        );
        ensure!(
            lines.next().is_some() && lines.next().is_some_and(|line| line.trim() == "^"),
            "javac warning is missing its source excerpt or caret"
        );
        warnings += 1;
        if let Some((rule, message)) = lint(message)? {
            budget.push(
                &mut data.issues,
                Issue {
                    rule: rule.into(),
                    file: Some(file.into()),
                    line: Some(number),
                    message: message.into(),
                    symbol: None,
                    tool: Some("javac".into()),
                    locations: Vec::new(),
                },
            )?;
        }
    }

    ensure!(
        summary == (warnings > 0).then_some(warnings),
        "javac warning summary does not match the diagnostic records"
    );
    Ok(data)
}

fn lint(message: &str) -> Result<Option<(&'static str, &str)>> {
    ensure!(!message.trim().is_empty(), "javac warning has no message");
    let Some(tag) = message.strip_prefix('[') else {
        return Ok(None);
    };
    let (label, message) = tag
        .split_once("] ")
        .context("Invalid javac warning label")?;
    ensure!(
        !message.trim().is_empty() && !label.is_empty(),
        "javac labeled warning has no label or message"
    );
    Ok(match label {
        "deprecation" => Some(("deprecation", message)),
        "removal" => Some(("removal", message)),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::parse;

    const DEPRECATED: &str = "src/red/Consumer.java:4: warning: [deprecation] Old in red has been deprecated\n  Old item;\n  ^\n";
    const REMOVAL: &str = "src/blue/Consumer.java:12: warning: [removal] Old in blue has been deprecated and marked for removal\n  @Old\n   ^\n";

    #[test]
    fn retains_paths_and_both_lint_kinds_and_ignores_other_warnings() {
        let report = format!(
            "warning: [options] system modules path not set in conjunction with -source 11\r\n{DEPRECATED}{REMOVAL}Note: unrelated compiler note\n3 warnings\n"
        );
        let data = parse(&report).unwrap();
        assert_eq!(data.issues.len(), 2);
        assert_eq!(data.issues[0].rule, "deprecation");
        assert_eq!(
            data.issues[0].file.as_deref(),
            Some("src/red/Consumer.java")
        );
        assert_eq!(data.issues[0].line, Some(4));
        assert_eq!(data.issues[1].rule, "removal");
        assert_eq!(
            data.issues[1].file.as_deref(),
            Some("src/blue/Consumer.java")
        );
        assert_eq!(data.issues[1].line, Some(12));
        assert_eq!(data.issues[1].tool.as_deref(), Some("javac"));
        assert!(parse("").unwrap().issues.is_empty());
    }

    #[test]
    fn rejects_incomplete_or_unrecognized_evidence() {
        for report in [
            " \n".to_owned(),
            DEPRECATED.trim_end().to_owned(),
            DEPRECATED.to_owned(),
            format!("{DEPRECATED}2 warnings\n"),
            format!("{DEPRECATED}1 warning\nonly showing the first 1 warnings, of 2 total\n"),
            format!("{}1 warning\n", DEPRECATED.replace("  ^\n", "")),
            format!("{}1 warning\n", DEPRECATED.replace(".java:4:", ".java:0:")),
            format!("{}1 warning\n", DEPRECATED.replace(".java:4:", ".java:x:")),
            "warning: [removal] Deprecated item\n1 warning\n".to_owned(),
            "Note: Some input files use or override a deprecated API.\n".to_owned(),
            "Note: Recompile with -Xlint:deprecation for details.\n".to_owned(),
            "Note: Recompile with -Xlint:removal for details.\n".to_owned(),
            format!(
                "{}1 warning\n",
                DEPRECATED.replace("[deprecation]", "[deprecation")
            ),
            "Consumer.java:1: error: cannot find symbol\n1 error\n".to_owned(),
            "Consumer.java:1:1: compiler.warn.has.been.deprecated: Old, pkg\n1 warning\n"
                .to_owned(),
        ] {
            assert!(parse(&report).is_err(), "{report:?}");
        }
    }
}
