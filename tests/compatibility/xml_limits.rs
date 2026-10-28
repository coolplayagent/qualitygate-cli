use super::*;

#[test]
fn bounded_compatibility_xml_retains_reports_and_blocks_dependent_checks() {
    let fixture = Fixture::new();
    let mut policy = fixture.policy.clone();
    policy["checks"].as_array_mut().unwrap().push(json!({
        "id":"dependent", "argv":["git","--version"], "depends_on":["api"]
    }));
    fixture.configure(&policy);
    for stage in ["inventory", "comparison"] {
        for (shape, tag, count, reason) in [
            ("deep", "<nested>", 2_000, "nesting exceeds"),
            ("wide", "<n/>", 100_000, "nodes limit reached"),
        ] {
            fixture.set_mode(&format!("{shape}-{stage}"));
            let value = fixture.run(&[], 2);
            assert_eq!(value["gate"]["complete"], false);
            let check = &value["checks"][0];
            // The compatibility workflow classifies unavailable comparison
            // evidence as blocked; it must not fabricate a completed verdict.
            assert_eq!(check["execution"]["status"], "blocked");
            assert!(
                check["execution"]["reason"]
                    .as_str()
                    .unwrap()
                    .contains(reason)
            );
            assert!(check["verdict"].is_null());
            assert!(check["metadata"]["compatibility"]["comparison"].is_null());
            assert!(check["diagnostics"].as_array().unwrap().is_empty());
            let suffix = if stage == "inventory" {
                "compatibility-inventory.xml"
            } else {
                "compatibility-api.xml"
            };
            let artifact = check["execution"]["artifacts"]
                .as_array()
                .unwrap()
                .iter()
                .find(|item| item["path"].as_str().unwrap().ends_with(suffix))
                .unwrap();
            let bytes = std::fs::read(artifact["path"].as_str().unwrap()).unwrap();
            assert_eq!(artifact["digest"], qualitygate::snapshot::digest(&bytes));
            assert_eq!(artifact["bytes"], bytes.len());
            let text = String::from_utf8(bytes).unwrap();
            assert_eq!(text.matches(tag).count(), count);
            assert!(text.ends_with("</japicmp>"));
            if stage == "comparison" {
                assert_eq!(check["metadata"]["compatibility"]["inventory_classes"], 1);
            }
            assert_eq!(value["checks"][1]["execution"]["status"], "blocked");
            assert!(value["checks"][1]["execution"]["started_at_ms"].is_null());
        }
    }
    fixture.set_mode("ok");
    let value = fixture.run(&[], 0);
    assert_eq!(value["gate"]["complete"], true);
    assert_eq!(
        value["checks"][0]["metadata"]["compatibility"]["comparison"]["classes_compared"],
        1
    );
    assert_eq!(value["checks"][1]["verdict"], "pass");
}
