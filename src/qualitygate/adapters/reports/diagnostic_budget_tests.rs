use super::{Data, parse};
use crate::config::ReportFormat;
use serde_json::json;

fn document(format: ReportFormat, files: &[(&str, usize)]) -> Vec<u8> {
    let text = if format == ReportFormat::EslintJson {
        let files: Vec<_> = files
            .iter()
            .map(|(path, count)| {
                let message = json!({"ruleId":"R","severity":2,"line":1,"message":"finding"});
                json!({"filePath":path,"fatalErrorCount":0,"errorCount":count,
                    "warningCount":0,"messages":vec![message; *count]})
            })
            .collect();
        serde_json::to_string(&files).unwrap()
    } else {
        let (root, finding) = if format == ReportFormat::Checkstyle {
            (
                "checkstyle",
                "<error source='R' line='1' message='finding'/>",
            )
        } else {
            (
                "pmd",
                "<violation rule='R' beginline='1'>finding</violation>",
            )
        };
        let files: String = files
            .iter()
            .map(|(path, count)| format!("<file name='{path}'>{}</file>", finding.repeat(*count)))
            .collect();
        format!("<{root}>{files}</{root}>")
    };
    assert!(text.len() < crate::snapshot::MAX_FILE_BYTES);
    text.into_bytes()
}

fn read(format: ReportFormat, files: &[(&str, usize)]) -> anyhow::Result<Data> {
    parse(format, &document(format, files))
}

fn oversized(format: ReportFormat, files: &[(&str, usize)]) {
    assert!(
        read(format, files)
            .unwrap_err()
            .to_string()
            .contains("Normalized diagnostics exceed 16 MiB")
    );
}

#[test]
fn shared_file_paths_are_bounded_before_retaining_all_diagnostics() {
    let path = "x".repeat(1_000_000);
    for format in [
        ReportFormat::EslintJson,
        ReportFormat::Checkstyle,
        ReportFormat::Pmd,
    ] {
        assert_eq!(read(format, &[(&path, 16)]).unwrap().issues.len(), 16);
        for count in [17, 1024] {
            oversized(format, &[(&path, count)]);
        }
    }
}

#[test]
fn diagnostic_budget_is_shared_across_files_and_counts_json_escaping() {
    let path = "x".repeat(1_000_000);
    let other = format!("{path}b");
    let escaped = "\\".repeat(65_536);
    for format in [
        ReportFormat::EslintJson,
        ReportFormat::Checkstyle,
        ReportFormat::Pmd,
    ] {
        assert_eq!(read(format, &[(&path, 9)]).unwrap().issues.len(), 9);
        oversized(format, &[(&path, 9), (&other, 9)]);
        assert_eq!(read(format, &[(&escaped, 127)]).unwrap().issues.len(), 127);
        oversized(format, &[(&escaped, 128)]);
    }
}
