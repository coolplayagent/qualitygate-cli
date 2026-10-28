use super::*;
use serde_json::Value;

fn initialize(root: &Path) -> Value {
    report(
        &cli(root, &["init", "--with-checks", "--format", "json"]),
        0,
    )
}

fn project<'a>(report: &'a Value, manifest: &str) -> &'a Value {
    report["discovery"]["projects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|project| project["manifest"] == manifest)
        .unwrap()
}

fn assert_gap(report: &Value, manifest: &str, reason: &str) {
    assert_eq!(report["status"], "candidate");
    assert_eq!(report["discovery"]["commands_executed"], false);
    let project = project(report, manifest);
    assert_eq!(project["metadata_status"], "invalid_or_unsupported");
    assert!(
        project["project_capabilities"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        report["discovery"]["gaps"]
            .as_array()
            .unwrap()
            .iter()
            .any(|gap| gap["path"] == manifest
                && gap["capability"] == "manifest_metadata"
                && gap["reason"].as_str().unwrap().contains(reason))
    );
    assert!(
        report["discovery"]["suggested_checks"]
            .as_array()
            .unwrap()
            .iter()
            .all(|suggestion| suggestion["source"] != manifest)
    );
}

#[test]
fn oversized_maven_xml_is_an_advisory_gap_and_recovers_without_rewriting_policy() {
    let deep = format!(
        "<project>{}{}</project>",
        "<n>".repeat(2_000),
        "</n>".repeat(2_000)
    );
    let wide = format!("<project>{}</project>", "<n/>".repeat(99_999));
    for (xml, reason) in [
        (deep.as_str(), "nesting exceeds"),
        (wide.as_str(), "nodes limit reached"),
        ("<project><n></project>", "Invalid Maven XML"),
    ] {
        let root = fixture();
        write(root.path(), "pom.xml", xml);
        let initialized = initialize(root.path());
        assert_gap(&initialized, "pom.xml", reason);
        assert_eq!(initialized["created"], true);
        assert_eq!(initialized["with_checks_applied"], false);
        assert!(
            initialized["config"]["checks"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            initialized["discovery"]["inputs"]["pom.xml"],
            qualitygate::snapshot::digest(xml.as_bytes())
        );
        assert_eq!(
            std::fs::read(root.path().join("pom.xml")).unwrap(),
            xml.as_bytes()
        );
        let candidate = std::fs::read(root.path().join("qualitygate.yaml")).unwrap();
        write(root.path(), "pom.xml", "<project/>");
        let repaired = initialize(root.path());
        assert_eq!(
            project(&repaired, "pom.xml")["metadata_status"],
            "inspected"
        );
        assert!(
            repaired["discovery"]["suggested_checks"]
                .as_array()
                .unwrap()
                .iter()
                .any(|suggestion| suggestion["source"] == "pom.xml")
        );
        assert!(
            !repaired["discovery"]["gaps"]
                .as_array()
                .unwrap()
                .iter()
                .any(|gap| gap["capability"] == "manifest_metadata")
        );
        assert_eq!(repaired["created"], false);
        assert_eq!(repaired["with_checks_applied"], false);
        assert_eq!(repaired["discovery"]["commands_executed"], false);
        assert_eq!(
            std::fs::read(root.path().join("qualitygate.yaml")).unwrap(),
            candidate
        );
    }
    let root = fixture();
    write(root.path(), "pom.xml", "<project/>");
    let valid = initialize(root.path());
    assert_eq!(valid["created"], true);
    assert_eq!(valid["with_checks_applied"], true);
    assert_eq!(valid["config"]["checks"].as_array().unwrap().len(), 1);
    assert_eq!(project(&valid, "pom.xml")["metadata_status"], "inspected");
}

#[test]
fn malformed_maven_manifest_does_not_hide_other_projects_or_replace_existing_policy() {
    let root = fixture();
    let existing = "schema_version: 1\nchecks: [{id: reviewed, argv: [git, --version]}]\n";
    write(root.path(), "qualitygate.yaml", existing);
    write(root.path(), "good/pom.xml", "<project/>");
    let deep = format!(
        "<project>{}{}</project>",
        "<n>".repeat(2_000),
        "</n>".repeat(2_000)
    );
    write(root.path(), "bad/pom.xml", &deep);
    let initialized = initialize(root.path());
    assert_gap(&initialized, "bad/pom.xml", "nesting exceeds");
    assert_eq!(
        project(&initialized, "good/pom.xml")["metadata_status"],
        "inspected"
    );
    assert!(
        initialized["discovery"]["suggested_checks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|suggestion| suggestion["source"] == "good/pom.xml")
    );
    assert_eq!(initialized["created"], false);
    assert_eq!(initialized["with_checks_applied"], false);
    assert_eq!(initialized["config"]["checks"][0]["id"], "reviewed");
    assert_eq!(
        std::fs::read(root.path().join("qualitygate.yaml")).unwrap(),
        existing.as_bytes()
    );
    write(root.path(), "bad/pom.xml", "<project/>");
    let repaired = initialize(root.path());
    assert_eq!(
        project(&repaired, "bad/pom.xml")["metadata_status"],
        "inspected"
    );
    assert_eq!(
        project(&repaired, "good/pom.xml")["metadata_status"],
        "inspected"
    );
    assert_eq!(repaired["config"]["checks"][0]["id"], "reviewed");
    assert_eq!(
        std::fs::read(root.path().join("qualitygate.yaml")).unwrap(),
        existing.as_bytes()
    );
}
