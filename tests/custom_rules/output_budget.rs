use super::*;
use serde_json::{Value, json};

fn install_output_rule(root: &Path, schema: u32, assertions: Value, fix: &str, expression: bool) {
    let mut rule = json!({
        "schema_version":schema,"id":"private-rule","version":1,
        "source":{"document":"AGENTS.md","section":"Team rules","content_hash":qualitygate::snapshot::digest(SOURCE.as_bytes())},
        "requires_capabilities":["files"],"when":{"entity":"file","change":"all"},
        "applies_to":{"paths":["data/*.txt"]},"then":assertions,"fix":fix
    });
    if schema >= 2 {
        rule["expression"] = json!({"op":"bool","value":expression});
    }
    std::fs::write(
        root.join("private-rules/private.yaml"),
        serde_norway::to_string(&rule).unwrap(),
    )
    .unwrap();
    reviews::record(root).unwrap();
}

#[test]
fn project_rule_output_exhaustion_is_incomplete_for_all_versions_and_repairable() {
    let temp = fixture();
    let root = temp.path();
    policy(root, "");
    std::fs::create_dir(root.join("data")).unwrap();
    for index in 0..64 {
        std::fs::write(root.join(format!("data/{index}.txt")), "bad\n").unwrap();
    }
    let fix = "Repair the selected files. ".repeat(10_000);
    for version in [1, 2, 3] {
        for assertions in [json!({"forbid_pattern":"bad"}), json!({"max_lines":0})] {
            install_output_rule(root, version, assertions, &fix, true);
            let blocked = check(root, 2);
            let result = &blocked["checks"][0];
            assert_eq!(blocked["gate"]["complete"], false);
            assert!(result["verdict"].is_null());
            assert_eq!(result["matched_entities"], 64);
            assert!(result["diagnostics"].as_array().unwrap().len() > 1);
            assert!(result["diagnostics"].as_array().unwrap().len() < 64);
            assert!(
                result["execution"]["reason"]
                    .as_str()
                    .unwrap()
                    .contains("Diagnostic output")
            );
            assert_eq!(result["metadata"]["diagnostic_output"]["complete"], false);
            assert!(serde_json::to_vec(&result["diagnostics"]).unwrap().len() <= 8 * 1024 * 1024);
        }
    }
    install_output_rule(
        root,
        3,
        json!({"forbid_pattern":"bad"}),
        "Remove bad text",
        true,
    );
    let failed = check(root, 1);
    assert_eq!(
        failed["checks"][0]["diagnostics"].as_array().unwrap().len(),
        64
    );
    for index in 0..64 {
        std::fs::write(root.join(format!("data/{index}.txt")), "good\n").unwrap();
    }
    check(root, 0);
}

#[test]
fn fixed_and_expression_findings_share_the_same_output_allowance() {
    let temp = fixture();
    let root = temp.path();
    policy(root, "");
    std::fs::create_dir(root.join("data")).unwrap();
    for index in 0..9 {
        std::fs::write(root.join(format!("data/{index}.txt")), "bad\n").unwrap();
    }
    let fix = "Repair these files. ".repeat(45_000); // 900,000 bytes, within the policy file limit.
    for version in [2, 3] {
        install_output_rule(root, version, json!({"forbid_pattern":"bad"}), &fix, false);
        let blocked = check(root, 2);
        let findings = blocked["checks"][0]["diagnostics"].as_array().unwrap();
        assert_eq!(findings.len(), 9);
        assert!(
            findings
                .iter()
                .all(|finding| finding["evidence"]["assertion"] == "forbid_pattern")
        );
        assert_eq!(
            blocked["checks"][0]["metadata"]["diagnostic_output"]["phase"],
            "rule_evaluation"
        );
    }
}

#[test]
fn clause_coverage_observes_output_exhaustion_during_recheck_binding() {
    let temp = fixture();
    let root = temp.path();
    policy(root, "");
    std::fs::create_dir(root.join("data")).unwrap();
    for index in 0..9 {
        std::fs::write(root.join(format!("data/{index}.txt")), "bad\n").unwrap();
    }
    install_output_rule(root, 3, json!({"forbid_pattern":"bad"}), "R", true);
    let baseline = check(root, 1);
    let mut raw = baseline["checks"][0]["diagnostics"].clone();
    for diagnostic in raw.as_array_mut().unwrap() {
        diagnostic["fix"] = json!("");
        diagnostic["recheck"]["argv"] = json!([]);
    }
    let overhead = serde_json::to_vec(&raw).unwrap().len();
    let fix_bytes = (qualitygate::domain::MAX_DIAGNOSTIC_BYTES - overhead) / 9;
    install_output_rule(
        root,
        3,
        json!({"forbid_pattern":"bad"}),
        &"R".repeat(fix_bytes),
        true,
    );
    let policy_path = root.join("qualitygate.yaml");
    let mut value: Value = serde_norway::from_slice(&std::fs::read(&policy_path).unwrap()).unwrap();
    value["constraints"] = json!([{
        "schema_version":1,"id":"files","version":1,"required":true,
        "statement":"Selected files must satisfy the project rule.",
        "source":{"document":"AGENTS.md","section":"Team rules","content_hash":qualitygate::snapshot::digest(SOURCE.as_bytes())},
        "realization":{"kind":"checks","mechanism":"regex","checks":["private-rule"],
            "positive_examples":["good text"],"negative_examples":["bad text"]}
    }]);
    std::fs::write(&policy_path, serde_norway::to_string(&value).unwrap()).unwrap();
    let config = report(&cli(root, &["config", "--show", "--format", "json"]), 0);
    value["constraints"][0]["adoption"] = json!({
        "id":"adopt-files","reviewer":"fixture reviewer","reference":"fixture review",
        "resolution":"initial_mapping","rationale":"Check scope and examples reviewed",
        "binding_digest":config["constraint_bindings"]["files"]
    });
    std::fs::write(&policy_path, serde_norway::to_string(&value).unwrap()).unwrap();
    let blocked = check(root, 2);
    let rule = &blocked["checks"][0];
    assert_eq!(rule["execution"]["status"], "blocked");
    assert_eq!(
        rule["metadata"]["diagnostic_output"]["phase"],
        "recheck_binding"
    );
    assert_eq!(rule["diagnostics"].as_array().unwrap().len(), 8);
    let coverage = blocked["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["id"] == "constraint-coverage")
        .unwrap();
    assert!(coverage["verdict"].is_null());
    assert_eq!(
        coverage["metadata"]["constraint_coverage"]["clauses"][0]["status"],
        "pending"
    );
}
