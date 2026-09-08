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
    let jacoco = parse(ReportFormat::Jacoco, br#"<report><package name="com/acme"><sourcefile name="A.java"><line nr="2" mi="1" ci="2" mb="1" cb="1"/></sourcefile></package></report>"#).unwrap();
    assert_eq!(jacoco.coverage[0].file, "com/acme/A.java");
    let cobertura = parse(ReportFormat::Cobertura, br#"<coverage><class filename="x.py"><lines><line number="3" hits="0" branch="true" condition-coverage="50% (1/2)"/></lines></class></coverage>"#).unwrap();
    assert_eq!(cobertura.coverage[0].branches_hit, 1);
    assert!(parse(ReportFormat::Lcov, b"SF:file\nDA:1,1").is_err());
}

#[test]
fn sarif_and_generic_diagnostics_validate_shape() {
    let sarif = parse(ReportFormat::Sarif, br#"{"version":"2.1.0","runs":[{"results":[{"ruleId":"r","message":{"text":"bad"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"src/x.rs"},"region":{"startLine":5}}}]}]}]}"#).unwrap();
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
