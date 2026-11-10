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

fn read_with_limit(
    format: ReportFormat,
    files: &[(&str, usize)],
    limit: usize,
) -> anyhow::Result<Data> {
    let bytes = document(format, files);
    let text = std::str::from_utf8(&bytes).unwrap();
    let budget = super::json_budget::Budget::with_limit("Normalized diagnostics", limit);
    if format == ReportFormat::EslintJson {
        super::eslint_json::parse_with_budget(text, budget)
    } else {
        super::xml::parse_with_budget(format, text, budget)
    }
}

fn issue_bytes(format: ReportFormat, path: &str) -> usize {
    serde_json::to_vec(&read(format, &[(path, 1)]).unwrap().issues[0])
        .unwrap()
        .len()
}

fn oversized(format: ReportFormat, files: &[(&str, usize)], limit: usize) {
    assert!(
        read_with_limit(format, files, limit)
            .unwrap_err()
            .to_string()
            .contains(&format!("Normalized diagnostics exceed {limit} bytes"))
    );
}

const FORMATS: [ReportFormat; 3] = [
    ReportFormat::EslintJson,
    ReportFormat::Checkstyle,
    ReportFormat::Pmd,
];

#[test]
fn shared_file_paths_are_bounded_before_retaining_all_diagnostics() {
    let path = "x".repeat(64);
    for format in FORMATS {
        let limit = issue_bytes(format, &path) * 16;
        assert_eq!(
            read_with_limit(format, &[(&path, 16)], limit)
                .unwrap()
                .issues
                .len(),
            16
        );
        for count in [17, 1024] {
            oversized(format, &[(&path, count)], limit);
        }
    }
}

#[test]
fn diagnostic_budget_is_shared_across_files_and_counts_json_escaping() {
    let path = "x".repeat(64);
    let other = format!("{path}b");
    let escaped = "\\".repeat(32);
    for format in FORMATS {
        let limit = (issue_bytes(format, &path) + issue_bytes(format, &other)) * 9 - 1;
        for file in [&path, &other] {
            assert_eq!(
                read_with_limit(format, &[(file, 9)], limit)
                    .unwrap()
                    .issues
                    .len(),
                9
            );
        }
        oversized(format, &[(&path, 9), (&other, 9)], limit);
        let limit = issue_bytes(format, &escaped) * 3 - 1;
        // Counting raw text instead of JSON escapes would incorrectly accept all three.
        assert!(issue_bytes(format, &"x".repeat(32)) * 3 <= limit);
        assert_eq!(
            read_with_limit(format, &[(&escaped, 2)], limit)
                .unwrap()
                .issues
                .len(),
            2
        );
        oversized(format, &[(&escaped, 3)], limit);
    }
}

#[test]
fn public_parsers_keep_the_real_sixteen_mib_default_boundary() {
    // Keep one full-size wiring check per format. The scenario matrix above
    // uses the same parser and accounting with a small explicit budget.
    let path = "x".repeat(1_000_000);
    for format in FORMATS {
        assert_eq!(read(format, &[(&path, 16)]).unwrap().issues.len(), 16);
        assert!(
            read(format, &[(&path, 17)])
                .unwrap_err()
                .to_string()
                .contains("Normalized diagnostics exceed 16 MiB")
        );
    }
}
