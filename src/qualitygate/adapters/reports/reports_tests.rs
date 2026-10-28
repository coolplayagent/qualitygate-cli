use super::*;

#[test]
fn junit_counts_executed_tests_instead_of_skipped_and_surfaces_assertions() {
    let data = parse(ReportFormat::Junit, br#"<testsuite tests="3"><testcase name="ok"/><testcase name="skip"><skipped/></testcase><testcase name="bad"><failure message="assertion failed"/></testcase></testsuite>"#).unwrap();
    let tests = data.tests.unwrap();
    assert_eq!((tests.executed, tests.skipped, tests.failures), (2, 1, 1));
    assert_eq!(data.issues[0].message, "assertion failed");
    assert!(parse(ReportFormat::Junit, br#"<testsuite tests="1"/>"#).is_err());
    assert_eq!(
        parse(ReportFormat::Junit, b"<testsuite/>")
            .unwrap()
            .tests
            .unwrap()
            .executed,
        0
    );
}

#[test]
fn junit_validates_optional_counters_in_every_suite() {
    let cases = "<testcase/><testcase><failure message='first'/><failure message='second'/></testcase><testcase><error/></testcase><testcase><skipped/></testcase>";
    let counters = "tests='4' failures='1' errors='1' skipped='1'";
    let valid = format!(
        "<testsuites {counters}><testsuite tests='0'/><testsuite {counters}>{cases}</testsuite></testsuites>"
    );
    let data = parse(ReportFormat::Junit, valid.as_bytes()).unwrap();
    let tests = data.tests.unwrap();
    assert_eq!((tests.executed, tests.failures, tests.skipped), (3, 2, 1));
    assert_eq!(data.issues.len(), 2);
    assert_eq!(data.issues[0].message, "first; second");
    assert!(data.issues.iter().all(|issue| issue.file.is_none()));

    for (attribute, count) in [("tests", 4), ("failures", 1), ("errors", 1), ("skipped", 1)] {
        for declared in [
            (count - 1).to_string(),
            (count + 1).to_string(),
            "invalid".into(),
            "-1".into(),
            "18446744073709551616".into(),
        ] {
            let suite = format!("<testsuite {attribute}='{declared}'>{cases}</testsuite>");
            for input in [
                suite.clone(),
                format!("<testsuites {counters}>{suite}</testsuites>"),
                format!(
                    "<testsuites {attribute}='{declared}'><testsuite>{cases}</testsuite></testsuites>"
                ),
            ] {
                let error = parse(ReportFormat::Junit, input.as_bytes())
                    .unwrap_err()
                    .to_string();
                assert!(error.contains(attribute), "{input}: {error}");
                if let Ok(value) = declared.parse::<usize>() {
                    assert!(error.contains(&format!("declared {value}, observed {count}")));
                }
            }
        }
    }
}

#[test]
fn junit_rejects_contradictory_outcomes_and_detached_records() {
    for outcomes in [
        "<failure/><skipped/>",
        "<skipped/><error/>",
        "<failure/><error/>",
        "<error/><error/>",
        "<skipped/><skipped/>",
    ] {
        let input = format!("<testsuite><testcase>{outcomes}</testcase></testsuite>");
        assert!(
            parse(ReportFormat::Junit, input.as_bytes())
                .unwrap_err()
                .to_string()
                .contains("contradictory outcomes"),
            "{input}"
        );
    }
    for input in [
        "<testsuite><failure/></testsuite>",
        "<testsuite><error/></testsuite>",
        "<testsuite><skipped/></testsuite>",
        "<testsuite><testcase><testcase/></testcase></testsuite>",
        "<testsuites><testcase/></testsuites>",
        "<testsuite><properties><testsuite/></properties></testsuite>",
        "<testsuite><testcase><properties><failure/></properties></testcase></testsuite>",
    ] {
        assert!(
            parse(ReportFormat::Junit, input.as_bytes()).is_err(),
            "{input}"
        );
    }
    // Producer extension records describe earlier attempts, not the final outcome.
    let rerun = b"<testsuite tests='1' failures='0'><testcase><flakyFailure/><rerunFailure/><system-out>retry succeeded</system-out></testcase></testsuite>";
    let data = parse(ReportFormat::Junit, rerun).unwrap();
    assert_eq!(data.tests.unwrap().failures, 0);
    assert!(data.issues.is_empty());
}

#[test]
fn junit_nested_summaries_count_each_case_once() {
    let input = |depth| {
        format!(
            "{}<testcase/>{}",
            "<testsuite tests='1' failures='0' errors='0' skipped='0'>".repeat(depth),
            "</testsuite>".repeat(depth)
        )
    };
    let tests = parse(
        ReportFormat::Junit,
        input(xml_limits::MAX_DEPTH - 1).as_bytes(),
    )
    .unwrap()
    .tests
    .unwrap();
    assert_eq!((tests.executed, tests.failures, tests.skipped), (1, 0, 0));
    for depth in [xml_limits::MAX_DEPTH, 2_000] {
        assert!(
            parse(ReportFormat::Junit, input(depth).as_bytes())
                .unwrap_err()
                .to_string()
                .contains("nesting exceeds")
        );
    }
}

#[test]
fn xml_reports_reject_excessive_nesting_before_tree_parsing() {
    for (format, root) in [
        (ReportFormat::Junit, "testsuite"),
        (ReportFormat::Checkstyle, "checkstyle"),
        (ReportFormat::Pmd, "pmd"),
        (ReportFormat::Spotbugs, "BugCollection"),
        (ReportFormat::Cobertura, "coverage"),
        (ReportFormat::Jacoco, "report"),
    ] {
        let input = format!(
            "<{root}>{}{}</{root}>",
            "<nested>".repeat(2_000),
            "</nested>".repeat(2_000)
        );
        assert!(
            parse(format, input.as_bytes())
                .unwrap_err()
                .to_string()
                .contains("nesting exceeds"),
            "{format:?}"
        );
    }
}

#[test]
fn static_xml_formats_keep_real_locations_and_reject_analyzer_errors() {
    let checkstyle = parse(ReportFormat::Checkstyle, br#"<checkstyle><file name="src/A.java"><error line="12" source="Naming" message="bad name"/></file></checkstyle>"#).unwrap();
    assert_eq!(checkstyle.issues[0].line, Some(12));
    let pmd = parse(ReportFormat::Pmd, br#"<pmd><file name="A.java"><violation beginline="3" rule="Unused"> unused </violation></file></pmd>"#).unwrap();
    assert_eq!(pmd.issues[0].message, "unused");
    assert!(parse(ReportFormat::Pmd, b"<pmd><error/></pmd>").is_err());
    let bugs = parse(ReportFormat::Spotbugs, br#"<BugCollection><BugInstance type="NP_NULL"><LongMessage>null access</LongMessage><SourceLine sourcepath="A.java" start="5"/></BugInstance></BugCollection>"#).unwrap();
    assert_eq!(bugs.issues[0].line, Some(5));
    assert!(
        parse(
            ReportFormat::Spotbugs,
            b"<BugCollection><Error/></BugCollection>"
        )
        .is_err()
    );
}

#[test]
fn coverage_parsers_preserve_uncovered_lines_and_branches() {
    let lcov = parse(
        ReportFormat::Lcov,
        b"SF:src/lib.rs\nDA:1,0\nDA:2,3\nBRDA:2,0,0,2\nBRDA:2,0,1,-\nend_of_record\n",
    )
    .unwrap();
    assert_eq!(lcov.coverage[0].hits, 0);
    assert_eq!(
        (
            lcov.coverage[1].branches_hit,
            lcov.coverage[1].branches_found
        ),
        (1, 2)
    );
    let counters = r#"<counter type="INSTRUCTION" missed="1" covered="2"/><counter type="LINE" missed="0" covered="1"/><counter type="BRANCH" missed="1" covered="1"/>"#;
    let jacoco = parse(ReportFormat::Jacoco, format!(r#"<report><package name="com/acme"><sourcefile name="A.java"><line nr="2" mi="1" ci="2" mb="1" cb="1"/>{counters}</sourcefile>{counters}</package>{counters}</report>"#).as_bytes()).unwrap();
    assert_eq!(jacoco.coverage[0].file, "com/acme/A.java");
    let cobertura = parse(ReportFormat::Cobertura, br#"<coverage><class filename="x.py"><lines><line number="3" hits="1" branch="true" condition-coverage="50% (1/2)"/></lines></class></coverage>"#).unwrap();
    assert_eq!(cobertura.coverage[0].branches_hit, 1);
    assert!(parse(ReportFormat::Lcov, b"SF:file\nDA:1,1").is_err());
}

#[test]
fn sarif_and_generic_diagnostics_validate_shape() {
    let sarif = parse(ReportFormat::Sarif, br#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"fixture"}},"results":[{"ruleId":"r","message":{"text":"bad"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"src/x.rs"},"region":{"startLine":5}}}]}]}]}"#).unwrap();
    assert_eq!(sarif.issues[0].file.as_deref(), Some("src/x.rs"));
    assert!(parse(ReportFormat::Sarif, br#"{"version":"2.1.0","runs":[]}"#).is_err());
    assert!(parse(ReportFormat::Diagnostics, b"{}").is_err());
    assert!(parse(ReportFormat::Diagnostics, br#"{"issues":[],"extra":true}"#).is_err());
    assert!(parse(ReportFormat::Diagnostics, br#"{"issues":[]}"#).is_ok());
    assert!(parse(ReportFormat::Checkstyle, b"<wrong/>").is_err());
    assert!(
        parse(
            ReportFormat::Checkstyle,
            b"<!DOCTYPE checkstyle><checkstyle/>"
        )
        .is_err()
    );
    assert!(parse(ReportFormat::Junit, b" ").is_err());
}

#[test]
fn lcov_sections_merge_by_identity_and_validate_source_summaries() {
    let first = "SF:src/a.rs\nDA:1,1\nBRDA:1,0,0,1\nBRDA:1,0,1,-\nLF:1\nLH:1\nBRF:2\nBRH:1\nend_of_record\n";
    let second = first
        .replace("BRDA:1,0,0,1", "BRDA:1,0,0,-")
        .replace("BRDA:1,0,1,-", "BRDA:1,0,1,1");
    let merged = parse(
        ReportFormat::Lcov,
        format!("TN:first\n{first}TN:second\n{second}").as_bytes(),
    )
    .unwrap();
    assert_eq!(merged.coverage.len(), 1);
    assert_eq!(merged.coverage[0].branches_found, 2);
    assert_eq!(merged.coverage[0].branches_hit, 2);
    assert!(merged.branch_coverage);
    for (key, declared, observed) in [("LF", 2, 1), ("LH", 0, 1), ("BRF", 3, 2), ("BRH", 2, 1)] {
        let inconsistent = first.replace("src/a.rs", "src/b.rs").replace(
            &format!("{key}:{observed}\n"),
            &format!("{key}:{declared}\n"),
        );
        let error = parse(
            ReportFormat::Lcov,
            format!("{first}{inconsistent}").as_bytes(),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains(&format!("LCOV {key} does not match source records")));
        assert!(error.contains("\"src/b.rs\""), "{error}");
        assert!(error.contains(&format!("declared {declared}, observed {observed}")));
    }
    for broken in [
        first.replace("DA:1,1", "DA:1,1\nDA:1,1"),
        first.replace("BRDA:1,0,1,-", "BRDA:1,0,0,1"),
        first.replace("DA:1,1", "DA:2,1"),
        "end_of_record\n".into(),
        "DA:1,1\n".into(),
        "SF:src/a.rs\nend_of_record\n".into(),
    ] {
        assert!(
            parse(ReportFormat::Lcov, broken.as_bytes()).is_err(),
            "{broken}"
        );
    }
    let empty = parse(
        ReportFormat::Lcov,
        b"SF:src/empty.rs\nLF:0\nLH:0\nBRF:0\nBRH:0\nend_of_record\n",
    )
    .unwrap();
    assert!(empty.coverage.is_empty());
    assert_eq!(empty.coverage_files, ["src/empty.rs"]);
    assert!(!empty.branch_coverage);
}

#[test]
fn coverage_and_test_summary_counters_cannot_disagree_with_detail_records() {
    let xml = br#"<coverage lines-valid="2" lines-covered="1" branches-valid="0" branches-covered="0"><class filename="a.py"><lines><line number="1" hits="1"/></lines></class></coverage>"#;
    assert!(parse(ReportFormat::Cobertura, xml).is_err());
    assert!(
        parse(
            ReportFormat::Diagnostics,
            br#"{"issues":[],"tests":{"executed":0,"failures":1,"skipped":0}}"#
        )
        .is_err()
    );
    assert!(parse(ReportFormat::Diagnostics, br#"{"issues":[],"coverage":[{"file":"a.rs","line":0,"hits":0,"branches_found":0,"branches_hit":0}]}"#).is_err());
}

#[test]
fn generic_location_inventory_preserves_ranges_and_cannot_contradict_primary_location() {
    use serde_json::json;
    let mut report = json!({"issues":[{"rule":"R","message":"A violation","file":"src/a.rs","line":2,"symbol":"fn",
        "tool":"scan","locations":[{"file":"src/a.rs","line":2,"end_line":3,"symbol":"fn"}]}]});
    let read = |value: &serde_json::Value| {
        parse(
            ReportFormat::Diagnostics,
            &serde_json::to_vec(value).unwrap(),
        )
    };
    assert_eq!(
        read(&report).unwrap().issues[0].locations[0].end_line,
        Some(3)
    );
    for (key, value) in [
        ("file", json!("different.rs")),
        ("line", json!(1)),
        ("symbol", json!("different")),
    ] {
        let mut broken = report.clone();
        broken["issues"][0]["locations"][0][key] = value;
        assert!(read(&broken).is_err());
    }
    report["issues"][0]["locations"]
        .as_array_mut()
        .unwrap()
        .push(json!({"file":"src/b.rs","line":0}));
    assert!(read(&report).is_err());
    report["issues"][0]["locations"][1] = json!({"file":"src/b.rs","line":2,"end_line":1});
    assert!(read(&report).is_err());
    report["issues"][0]["locations"][1] = json!({"file":"src/b.rs","end_line":1});
    assert!(read(&report).is_err());
}
