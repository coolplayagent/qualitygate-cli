use super::*;
use serde_json::{Value, json};

const DECLARATION: &str =
    r#"<!DOCTYPE report PUBLIC "-//JACOCO//DTD Report 1.1//EN" "report.dtd">"#;
const COUNTERS: &str = r#"<counter type="INSTRUCTION" missed="1" covered="2"/><counter type="LINE" missed="1" covered="1"/><counter type="BRANCH" missed="1" covered="1"/>"#;
const LINES: &str = r#"<line nr="2" ci="2" mb="1" cb="1"/><line nr="3" mi="1"/>"#;

fn jacoco() -> String {
    format!(
        r#"<report><group name="module"><package name="com/acme"><sourcefile name="A.java">{LINES}{COUNTERS}</sourcefile>{COUNTERS}</package>{COUNTERS}</group>{COUNTERS}</report>"#
    )
}

#[test]
fn known_dtds_are_inert_and_xml_entities_or_unknown_headers_are_rejected() {
    let body = jacoco();
    for declaration in [
        DECLARATION.to_owned(),
        DECLARATION.replace('"', "'"),
        DECLARATION.replace(" PUBLIC ", "\n PUBLIC\t"),
    ] {
        let text = format!(
            "\u{feff}<?xml version='1.0'?>\n<!-- <!DOCTYPE ignored> -->\n{declaration}{body}"
        );
        let normalized = xml_header::normalize(ReportFormat::Jacoco, &text).unwrap();
        assert_eq!(normalized.len(), text.len());
        assert_eq!(normalized.lines().count(), text.lines().count());
        assert_eq!(
            parse(ReportFormat::Jacoco, text.as_bytes())
                .unwrap()
                .coverage
                .len(),
            2
        );
    }
    for url in [
        "http://cobertura.sourceforge.net/xml/coverage-04.dtd",
        "https://cobertura.sourceforge.net/xml/coverage-04.dtd",
        "https://raw.githubusercontent.com/cobertura/web/master/htdocs/xml/coverage-04.dtd",
    ] {
        let text = format!(
            r#"<!DOCTYPE coverage SYSTEM "{url}"><coverage><class filename="empty.py"><lines/></class></coverage>"#
        );
        assert!(parse(ReportFormat::Cobertura, text.as_bytes()).is_ok());
    }
    for header in [
        "<!DOCTYPE report>".to_owned(),
        "<!DOCTYPE report SYSTEM 'file:///etc/passwd'>".into(),
        DECLARATION.replace("report.dtd", "https://example.invalid/report.dtd"),
        DECLARATION.replace("report PUBLIC", "coverage PUBLIC"),
        DECLARATION.replace("1.1//EN", "2.0//EN"),
        DECLARATION.replace(" PUBLIC ", " PUBLIC"),
        DECLARATION.replace(" PUBLIC ", " INVALID "),
        DECLARATION.replace("\"report.dtd\"", "report.dtd"),
        DECLARATION.replace('>', " [<!ENTITY read SYSTEM 'file:///etc/passwd'>]>"),
        DECLARATION.replace('>', " [<!ENTITY a 'x'><!ENTITY b '&a;&a;'>]>"),
        format!("{DECLARATION}{DECLARATION}"),
        DECLARATION.replace(" PUBLIC", &format!("{}PUBLIC", " ".repeat(1024))),
        "<?unclosed".into(),
        "<!--unclosed".into(),
        "<!DOCTYPE report PUBLIC 'unclosed".into(),
    ] {
        assert!(
            parse(ReportFormat::Jacoco, format!("{header}{body}").as_bytes()).is_err(),
            "{header}"
        );
    }
    let entity = format!("{DECLARATION}{}", body.replace("A.java", "&read;"));
    assert!(parse(ReportFormat::Jacoco, entity.as_bytes()).is_err());
    assert!(
        parse(
            ReportFormat::Junit,
            format!("{DECLARATION}<testsuite/>").as_bytes()
        )
        .is_err()
    );
    let nodes = format!("<report>{}</report>", "<x/>".repeat(100_001));
    assert!(parse(ReportFormat::Jacoco, nodes.as_bytes()).is_err());
}

#[test]
fn jacoco_rollups_detect_removed_sources_missing_debug_info_and_invalid_counters() {
    let valid = jacoco();
    let data = parse(ReportFormat::Jacoco, valid.as_bytes()).unwrap();
    assert_eq!(data.coverage_files, ["com/acme/A.java"]);
    assert!(data.branch_coverage);
    assert_eq!(data.coverage[0].branches_found, 2);
    assert_eq!(data.coverage[1].hits, 0);
    for broken in [
        valid.replacen(COUNTERS, "", 1),
        valid.replacen("covered=\"2\"", "covered=\"3\"", 1),
        valid.replacen(COUNTERS, &format!("{COUNTERS}{COUNTERS}"), 1),
        valid.replacen("type=\"LINE\"", "type=\"UNKNOWN\"", 1),
        valid.replace(LINES, ""),
        valid.replace(LINES, &format!("{LINES}{LINES}")),
        valid.replace("nr=\"2\"", "nr=\"0\""),
        valid.replace("ci=\"2\"", "ci=\"0\""),
        valid.replace("ci=\"2\"", "ci=\"0\" mi=\"2\""),
        valid.replace("<sourcefile name=\"A.java\">", "<sourcefile>"),
        valid.replace("<package name=\"com/acme\">", "<package>"),
        valid.replace("<sourcefile", "<other><sourcefile").replace("</sourcefile>", "</sourcefile></other>"),
        valid.replace("</report>", &format!("{LINES}</report>")),
        valid.replace("<group", "<wrong").replace("</group>", "</wrong>"),
        valid.replace("</package>", "<sourcefile name=\"A.java\"/></package>"),
        valid.replace("mi=\"1\"", &format!("mi=\"{}\"", usize::MAX)).replace("ci=\"2\"", "ci=\"2\" mi=\"1\""),
        "<report><package name='p'><class name='p/A'/><counter type='LINE' missed='1' covered='0'/></package></report>".into(),
        format!("<report>{}<package name='p'><sourcefile name='A.java'/></package>{}</report>", "<group>".repeat(32), "</group>".repeat(32)),
    ] {
        assert!(parse(ReportFormat::Jacoco, broken.as_bytes()).is_err(), "{broken}");
    }
    let empty = "<report><package name=''><sourcefile name='Empty.java'/></package></report>";
    assert_eq!(
        parse(ReportFormat::Jacoco, empty.as_bytes())
            .unwrap()
            .coverage_files,
        ["Empty.java"]
    );
}

fn cobertura() -> String {
    r#"<coverage lines-valid="1" lines-covered="1" branches-valid="3" branches-covered="2"><sources><source>src</source></sources><packages><package><classes><class filename="calc.py"><methods><method><lines><line number="2" hits="1"/></lines></method></methods><lines><line number="2" hits="1" branch="true" condition-coverage="66% (2/3)"/></lines></class></classes></package></packages></coverage>"#.into()
}

#[test]
fn cobertura_counts_and_source_inventory_preserve_branch_measurement_uncertainty() {
    let valid = cobertura();
    let data = parse(ReportFormat::Cobertura, valid.as_bytes()).unwrap();
    assert_eq!(data.coverage_roots, ["src"]);
    assert_eq!(data.coverage.len(), 1);
    assert!(data.branch_coverage);
    assert_eq!(data.coverage[0].branches_hit, 2);
    for broken in [
        valid.replace("66% (2/3)", "100% (2/3)"),
        valid.replace("66% (2/3)", "NaN% (2/3)"),
        valid.replace("66% (2/3)", "0% (0/0)"),
        valid.replace("66% (2/3)", "66% (4/3)"),
        valid.replace("66% (2/3)", "66% (2/3"),
        valid.replace("hits=\"1\"", "hits=\"0\""),
        valid.replace("branch=\"true\"", "branch=\"false\""),
        valid.replace("branch=\"true\"", "branch=\"unknown\""),
        valid.replace(" condition-coverage=\"66% (2/3)\"", ""),
        valid.replace("<source>src</source>", "<source/>"),
        valid.replace("<source>src</source>", &"<source>src</source>".repeat(65)),
        valid.replace("<class filename=\"calc.py\">", "<class>"),
        valid.replace("number=\"2\"", "number=\"0\""),
        valid.replace("</coverage>", "<line number='1' hits='0'/></coverage>"),
        valid.replace("branches-valid=\"3\"", "branches-valid=\"0\""),
        "<coverage/>".into(),
    ] {
        assert!(
            parse(ReportFormat::Cobertura, broken.as_bytes()).is_err(),
            "{broken}"
        );
    }
    let line_only = r#"<coverage branches-valid="0" branches-covered="0"><class filename="a.py"><lines><line number="1" hits="1" branch="false"/></lines></class></coverage>"#;
    assert!(
        !parse(ReportFormat::Cobertura, line_only.as_bytes())
            .unwrap()
            .branch_coverage
    );
}

fn python() -> Value {
    let summary = json!({"covered_lines":3,"num_statements":4,"missing_lines":1,"excluded_lines":1,
        "num_branches":2,"num_partial_branches":1,"covered_branches":1,"missing_branches":1});
    json!({"meta":{"format":3,"version":"7.10.7","branch_coverage":true},"totals":summary,
        "files":{"src/calc.py":{"executed_lines":[1,2,3,5],"missing_lines":[4],"excluded_lines":[5],
        "executed_branches":[[2,3]],"missing_branches":[[2,4]],"summary":summary}}})
}

fn read_python(report: &Value) -> Result<Data> {
    parse(
        ReportFormat::CoveragePy,
        &serde_json::to_vec(report).unwrap(),
    )
}

#[test]
fn native_python_exclusions_signed_branches_and_summaries_are_consistent() {
    let valid = python();
    let data = read_python(&valid).unwrap();
    assert!(data.branch_coverage);
    assert_eq!(data.coverage_producer.unwrap()["version"], "7.10.7");
    assert_eq!(data.coverage[4].hits, 0);
    assert!(data.coverage[4].excluded);
    for (key, value) in [
        ("executed_lines", json!([1, 2, 2, 3])),
        ("executed_lines", json!([0, 1, 2, 3])),
        ("executed_lines", json!([1, 2, 3, 4])),
        ("excluded_lines", json!([4, 5])),
        ("executed_branches", json!([[2, 3], [2, 3]])),
        ("executed_branches", json!([[2, 4]])),
        ("executed_branches", json!([[5, 3]])),
        ("executed_branches", json!([[4, 3]])),
        ("executed_branches", json!([[-2, 3]])),
        ("executed_branches", json!([[2, 0]])),
        ("executed_branches", json!([[2, i64::MIN]])),
        ("executed_branches", json!([[2, 99]])),
        ("executed_branches", json!(null)),
    ] {
        let mut broken = valid.clone();
        broken["files"]["src/calc.py"][key] = value;
        assert!(read_python(&broken).is_err(), "{broken}");
    }
    for key in [
        "covered_lines",
        "num_statements",
        "missing_lines",
        "excluded_lines",
        "num_branches",
        "num_partial_branches",
        "covered_branches",
        "missing_branches",
    ] {
        for pointer in ["/totals", "/files/src~1calc.py/summary"] {
            let mut broken = valid.clone();
            broken.pointer_mut(pointer).unwrap()[key] = json!(99);
            assert!(read_python(&broken).is_err(), "{broken}");
        }
    }
    for (key, value) in [
        ("format", json!(1)),
        ("version", json!(" ")),
        ("branch_coverage", json!(false)),
    ] {
        let mut broken = valid.clone();
        broken["meta"][key] = value;
        assert!(read_python(&broken).is_err());
    }
    let mut missing = valid.clone();
    missing["meta"]
        .as_object_mut()
        .unwrap()
        .remove("branch_coverage");
    assert!(read_python(&missing).is_err());
    missing["files"] = json!({});
    assert!(read_python(&missing).is_err());
    let mut signed = valid;
    signed["meta"]["format"] = json!(2);
    signed["files"]["src/calc.py"]["executed_branches"] = json!([[2, -1]]);
    assert!(read_python(&signed).is_ok());
}

#[test]
fn native_python_measured_zero_branches_differs_from_disabled_measurement() {
    let summary = json!({"covered_lines":0,"num_statements":0,"missing_lines":0,"excluded_lines":1,
        "num_branches":0,"num_partial_branches":0,"covered_branches":0,"missing_branches":0});
    let mut report = json!({"meta":{"format":3,"version":"7.10.7","branch_coverage":true},"totals":summary,
        "files":{"src/empty.py":{"executed_lines":[],"missing_lines":[],"excluded_lines":[1],
        "executed_branches":[],"missing_branches":[],"summary":summary}}});
    let measured = read_python(&report).unwrap();
    assert!(measured.branch_coverage);
    assert!(!measured.has_findings());
    report["meta"]["branch_coverage"] = json!(false);
    assert!(read_python(&report).is_err());
    for key in [
        "num_branches",
        "num_partial_branches",
        "covered_branches",
        "missing_branches",
    ] {
        report["totals"].as_object_mut().unwrap().remove(key);
        report["files"]["src/empty.py"]["summary"]
            .as_object_mut()
            .unwrap()
            .remove(key);
    }
    assert!(read_python(&report).is_err());
    for key in ["executed_branches", "missing_branches"] {
        report["files"]["src/empty.py"]
            .as_object_mut()
            .unwrap()
            .remove(key);
    }
    assert!(!read_python(&report).unwrap().branch_coverage);
    report["totals"]["excluded_lines"] = json!(0);
    report["files"]["src/empty.py"]["summary"]["excluded_lines"] = json!(0);
    report["files"]["src/empty.py"]["executed_lines"] = json!([0]);
    report["files"]["src/empty.py"]["excluded_lines"] = json!([]);
    assert!(read_python(&report).unwrap().coverage.is_empty());
    report["files"]["src/empty.py"]["executed_lines"] = json!([0, 1]);
    assert!(read_python(&report).is_err());
}

#[test]
fn native_python_repeated_filenames_and_missing_inventory_cannot_erase_evidence() {
    let valid = python();
    let duplicate = format!(
        r#"{{"meta":{},"totals":{},"files":{{"a.py":{},"a.py":{}}}}}"#,
        valid["meta"],
        valid["totals"],
        valid["files"]["src/calc.py"],
        valid["files"]["src/calc.py"]
    );
    assert!(
        parse(ReportFormat::CoveragePy, duplicate.as_bytes())
            .unwrap_err()
            .to_string()
            .contains("Duplicate")
    );
    let mut empty = valid;
    empty["files"] = json!({});
    assert!(read_python(&empty).is_err());
}

#[test]
fn repeated_long_filenames_cannot_expand_normalized_coverage_without_bound() {
    let file = format!("{}.rs", "a".repeat(8000));
    let lcov = format!(
        "SF:{file}\n{}end_of_record\n",
        (1..1000)
            .map(|line| format!("DA:{line},1\n"))
            .collect::<String>()
    );
    let xml = format!(
        "<coverage><class filename='{file}'><lines>{}</lines></class></coverage>",
        (1..1000)
            .map(|line| format!("<line number='{line}' hits='1'/>"))
            .collect::<String>()
    );
    let java = format!(
        "<report><package name=''><sourcefile name='{file}'>{}</sourcefile></package></report>",
        (1..1000)
            .map(|line| format!("<line nr='{line}' ci='1'/>"))
            .collect::<String>()
    );
    for (format, input) in [
        (ReportFormat::Lcov, lcov),
        (ReportFormat::Cobertura, xml),
        (ReportFormat::Jacoco, java),
    ] {
        assert!(
            parse(format, input.as_bytes())
                .unwrap_err()
                .to_string()
                .contains("16 MiB")
        );
    }
    let mut budget = 1024;
    for invalid in ["", "a\nb", "a\0b"] {
        assert!(coverage_budget(&mut budget, invalid).is_err());
    }
    assert!(coverage_budget(&mut budget, &"a".repeat(16385)).is_err());
    let data = json!({"issues":[],"coverage":[{"file":"a.py","line":1,"hits":1,"branches_found":0,"branches_hit":0,"excluded":true}]});
    assert!(
        parse(
            ReportFormat::Diagnostics,
            &serde_json::to_vec(&data).unwrap()
        )
        .is_err()
    );
}
