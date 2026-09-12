use super::*;
use std::io::Write;
use zip::{ZipWriter, write::SimpleFileOptions};

fn jar(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut archive = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in entries {
        archive
            .start_file(*name, SimpleFileOptions::default())
            .unwrap();
        archive.write_all(bytes).unwrap();
    }
    archive.finish().unwrap().into_inner()
}

fn xml(body: &str) -> String {
    format!(
        r#"<japicmp oldJar="old.jar" newJar="new.jar" accessModifier="PRIVATE" packagesInclude="all" packagesExclude="n.a." ignoreMissingClasses="false" ignoreMissingClassesByRegularExpressions="" onlyModifications="false" onlyBinaryIncompatibleModifications="false"><classes>{body}</classes></japicmp>"#
    )
}

const CLASS: &str =
    r#"<class fullyQualifiedName="Api" binaryCompatible="true" sourceCompatible="true"/>"#;

fn compare(report: &str, level: CompatibilityLevel) -> Result<Comparison> {
    parse(
        report.as_bytes(),
        "old.jar",
        "new.jar",
        &BTreeSet::from(["Api".into()]),
        level,
    )
}

#[test]
fn complete_inventory_and_unfiltered_report_are_required() {
    let report = xml(CLASS);
    assert_eq!(
        compare(&report, CompatibilityLevel::Both)
            .unwrap()
            .classes_compared,
        1
    );
    for (from, to) in [
        ("old.jar", "foreign.jar"),
        ("new.jar", "foreign.jar"),
        ("PRIVATE", "PUBLIC"),
        ("packagesInclude=\"all\"", "packagesInclude=\"Api\""),
        ("packagesExclude=\"n.a.\"", "packagesExclude=\"private.*\""),
        (
            "ignoreMissingClasses=\"false\"",
            "ignoreMissingClasses=\"true\"",
        ),
        (
            "ignoreMissingClassesByRegularExpressions=\"\"",
            "ignoreMissingClassesByRegularExpressions=\".*\"",
        ),
        ("onlyModifications=\"false\"", "onlyModifications=\"true\""),
        (
            "onlyBinaryIncompatibleModifications=\"false\"",
            "onlyBinaryIncompatibleModifications=\"true\"",
        ),
        ("binaryCompatible=\"true\"", "binaryCompatible=\"yes\""),
        ("sourceCompatible=\"true\"", ""),
        ("fullyQualifiedName=\"Api\"", "fullyQualifiedName=\"\""),
        ("fullyQualifiedName=\"Api\"", "fullyQualifiedName=\"Other\""),
        ("<class ", "<unknown "),
        ("</classes>", "</classes><classes/>"),
        ("<japicmp ", "<unknown "),
        ("<classes>", "<missing>"),
    ] {
        assert!(
            compare(&report.replace(from, to), CompatibilityLevel::Both).is_err(),
            "{from}"
        );
    }
    for body in [String::new(), format!("{CLASS}{CLASS}")] {
        assert!(compare(&xml(&body), CompatibilityLevel::Both).is_err());
    }
    assert!(
        parse(
            &vec![b' '; snapshot::MAX_FILE_BYTES + 1],
            "old.jar",
            "new.jar",
            &BTreeSet::new(),
            CompatibilityLevel::Both
        )
        .is_err()
    );
    assert!(
        parse(
            &[0xff],
            "old.jar",
            "new.jar",
            &BTreeSet::new(),
            CompatibilityLevel::Both
        )
        .is_err()
    );
}

#[test]
fn member_findings_preserve_policy_and_stable_symbol_identity() {
    let change = r#"<compatibilityChange type="GENERICS_CHANGED" binaryCompatible="true" sourceCompatible="false"/>"#;
    let body = format!(
        r#"<class fullyQualifiedName="Api" binaryCompatible="true" sourceCompatible="false"><methods><method name="get"><parameters><parameter type="java.lang.String"/></parameters><compatibilityChanges>{change}{change}</compatibilityChanges></method></methods><fields><field name="value"><compatibilityChanges>{change}</compatibilityChanges></field></fields></class>"#
    );
    let report = xml(&body);
    let both = compare(&report, CompatibilityLevel::Both).unwrap();
    assert!(both.binary_compatible);
    assert!(!both.source_compatible);
    assert_eq!(both.findings.len(), 2);
    assert_eq!(both.findings[0].symbol, "Api#get(java.lang.String)");
    assert_eq!(both.findings[1].symbol, "Api#value");
    assert!(
        compare(&report, CompatibilityLevel::Binary)
            .unwrap()
            .findings
            .is_empty()
    );
    assert_eq!(
        compare(&report, CompatibilityLevel::Source)
            .unwrap()
            .findings
            .len(),
        2
    );
    for (from, to) in [
        ("type=\"GENERICS_CHANGED\"", "type=\"\""),
        ("<parameters>", "<omitted>"),
        ("name=\"get\"", ""),
        ("type=\"java.lang.String\"", ""),
    ] {
        assert!(compare(&report.replace(from, to), CompatibilityLevel::Both).is_err());
    }
    let contradictory =
        report.replacen("sourceCompatible=\"false\"", "sourceCompatible=\"true\"", 1);
    assert!(compare(&contradictory, CompatibilityLevel::Both).is_err());
    let fallback = xml(&CLASS.replace("binaryCompatible=\"true\"", "binaryCompatible=\"false\""));
    assert_eq!(
        compare(&fallback, CompatibilityLevel::Both)
            .unwrap()
            .findings[0]
            .change,
        "CLASS_INCOMPATIBLE"
    );
    let class_change = xml(&format!(
        r#"<class fullyQualifiedName="Api" binaryCompatible="true" sourceCompatible="false"><compatibilityChanges>{change}</compatibilityChanges></class>"#
    ));
    assert_eq!(
        compare(&class_change, CompatibilityLevel::Both)
            .unwrap()
            .findings[0]
            .symbol,
        "Api"
    );
}

#[test]
fn analyzer_identity_is_digest_pinned_and_versioned() {
    const NAME: &str = "META-INF/maven/com.github.siom79.japicmp/japicmp/pom.properties";
    const PROPERTIES: &str =
        "# version\ngroupId=com.github.siom79.japicmp\nartifactId=japicmp\nversion=0.26.2\n";
    let bytes = jar(&[(NAME, PROPERTIES.as_bytes())]);
    assert_eq!(
        analyzer_version(&bytes, &snapshot::digest(&bytes)).unwrap(),
        "0.26.2"
    );
    assert!(analyzer_version(&bytes, "sha256:incorrect").is_err());
    for properties in [
        " ".repeat(16_385),
        PROPERTIES.replace("0.26.2", "0.26.1"),
        PROPERTIES.replace("artifactId=japicmp", "artifactId=other"),
        PROPERTIES.replace("groupId=com.github.siom79.japicmp", "groupId=other"),
        format!("{PROPERTIES}version=0.26.2"),
        format!("{PROPERTIES}invalid"),
    ] {
        let bytes = jar(&[(NAME, properties.as_bytes())]);
        assert!(analyzer_version(&bytes, &snapshot::digest(&bytes)).is_err());
    }
    let missing = jar(&[("META-INF/MANIFEST.MF", b"Manifest-Version: 1.0")]);
    assert!(analyzer_version(&missing, &snapshot::digest(&missing)).is_err());
}

#[test]
fn archives_reject_ambiguous_classes_unsupported_variants_and_unsafe_paths() {
    let bytes = jar(&[("com/Api.class", b"class"), ("data.txt", b"data")]);
    assert_eq!(classes(&bytes).unwrap(), BTreeSet::from(["com.Api".into()]));
    for name in [
        "../escape.class",
        "com\\Api.class",
        "META-INF/versions/21/Api.class",
        "module-info.class",
        "META-INF/Api.class",
        ".class",
    ] {
        assert!(classes(&jar(&[(name, b"class")])).is_err(), "{name}");
    }
    assert!(
        classes(&jar(&[
            ("com/Api.class", b"class"),
            ("com.Api.class", b"class")
        ]))
        .is_err()
    );
    assert!(classes(b"invalid archive").is_err());
    assert!(classes(&vec![0; MAX_ARCHIVE_BYTES + 1]).is_err());
    assert!(classes(&jar(&[("data.txt", b"data")])).unwrap().is_empty());
}
