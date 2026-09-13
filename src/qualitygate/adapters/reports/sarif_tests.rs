use super::*;
use serde_json::json;

fn report(results: Value) -> Value {
    json!({"version":"2.1.0","runs":[{
        "tool":{"driver":{"name":"fixture","semanticVersion":"1.0.0","rules":[{"id":"R","messageStrings":{"message":{"text":"Replace {0} with {{safe}}"}}}]}},
        "invocations":[{"executionSuccessful":true}], "results":results
    }]})
}

fn issue() -> Value {
    json!({"ruleId":"R","message":{"text":"A violation"},
        "locations":[{"physicalLocation":{"artifactLocation":{"uri":"src/a%20b.rs"},"region":{"startLine":2,"endLine":4}}}]})
}

fn parse_value(value: &Value) -> Result<Data> {
    parse(ReportFormat::Sarif, &serde_json::to_vec(value)?)
}

#[test]
fn successful_runs_resolve_rule_messages_file_tables_and_uri_bases() {
    let mut issue = issue();
    issue.as_object_mut().unwrap().remove("ruleId");
    issue["rule"] = json!({"index":0});
    issue["message"] = json!({"id":"message","arguments":["old"]});
    issue["locations"][0]["physicalLocation"]["artifactLocation"] = json!({"index":0});
    issue["locations"][0]["logicalLocations"] = json!([{"index":0}]);
    let mut value = report(json!([issue]));
    let run = &mut value["runs"][0];
    run["artifacts"] = json!([{"location":{"uri":"a%20b.rs","uriBaseId":"SRC"}}]);
    run["originalUriBaseIds"] =
        json!({"ROOT":{"uri":"source/"},"SRC":{"uri":"src/","uriBaseId":"ROOT"}});
    run["logicalLocations"] = json!([{"fullyQualifiedName":"module::function"}]);
    let data = parse_value(&value).unwrap();
    let issue = &data.issues[0];
    assert_eq!(issue.file.as_deref(), Some("source/src/a b.rs"));
    assert_eq!(issue.message, "Replace old with {safe}");
    assert_eq!(issue.symbol.as_deref(), Some("module::function"));
    assert_eq!(issue.locations[0].end_line, Some(4));
    assert_eq!(data.sarif_runs[0].tool, "fixture");
    assert_eq!(data.sarif_runs[0].version.as_deref(), Some("1.0.0"));
    let root = tempfile::tempdir().unwrap();
    let uri = Url::from_directory_path(root.path()).unwrap();
    value["runs"][0]["originalUriBaseIds"]["ROOT"]["uri"] = json!(uri);
    let data = parse_value(&value).unwrap();
    assert_eq!(
        Path::new(data.issues[0].file.as_ref().unwrap()),
        root.path().join("src/a b.rs")
    );
}

use std::path::Path;
use url::Url;

#[test]
fn malformed_invocations_external_results_and_unresolved_analysis_are_incomplete() {
    for (key, value) in [
        ("invocations", json!({})),
        ("invocations", json!([{}])),
        ("invocations", json!([{"executionSuccessful":false}])),
        ("invocations", json!([{"executionSuccessful":"true"}])),
        (
            "invocations",
            json!([{"executionSuccessful":true,"toolExecutionNotifications":[{"level":"error"}]}]),
        ),
        (
            "invocations",
            json!([{"executionSuccessful":true,"toolConfigurationNotifications":[{"exception":{}}]}]),
        ),
        (
            "invocations",
            json!([{"executionSuccessful":true,"toolExecutionNotifications":[{"level":"invalid"}]}]),
        ),
        (
            "externalPropertyFileReferences",
            json!({"results":[{"location":{"uri":"more.sarif"}}]}),
        ),
        ("results", Value::Null),
        ("results", json!({})),
        ("tool", json!({"driver":{}})),
        ("baselineGuid", json!("previous")),
    ] {
        let mut document = report(json!([]));
        document["runs"][0][key] = value;
        assert!(parse_value(&document).is_err(), "{key}");
    }
    for kind in ["open", "review", "unknown"] {
        let mut finding = issue();
        finding["kind"] = json!(kind);
        assert!(parse_value(&report(json!([finding]))).is_err());
    }
    let mut document = report(json!([]));
    document["inlineExternalProperties"] = json!([{}]);
    assert!(parse_value(&document).is_err());
    document = report(json!([]));
    document["runs"][0]
        .as_object_mut()
        .unwrap()
        .remove("results");
    assert!(parse_value(&document).is_err());
    for bad in [
        json!(null),
        json!([]),
        json!({"version":"1.0.0"}),
        json!({"version":"2.1.0","runs":[]}),
    ] {
        assert!(parse_value(&bad).is_err());
    }
    let mut document = report(json!([]));
    document["runs"][0]["invocations"] =
        json!([{"executionSuccessful":true,"toolExecutionNotifications":[{"level":"warning"}]}]);
    assert!(parse_value(&document).is_ok());
    document["runs"][0]
        .as_object_mut()
        .unwrap()
        .remove("invocations");
    assert!(parse_value(&document).is_ok());
}

#[test]
fn non_violations_are_counted_and_suppressions_do_not_silently_waive_policy() {
    let mut results = vec![issue()];
    for kind in ["pass", "informational", "notApplicable"] {
        let mut value = issue();
        value["kind"] = json!(kind);
        value["level"] = json!("none");
        results.push(value);
    }
    results[0]["suppressions"] = json!([{"kind":"inSource","status":"accepted"}]);
    let data = parse_value(&report(json!(results))).unwrap();
    assert_eq!(data.issues.len(), 1);
    assert_eq!(data.sarif_runs[0].non_violations, 3);
    assert_eq!(data.sarif_runs[0].suppressed_results, 1);
    for (key, value) in [
        ("level", json!("unknown")),
        ("kind", json!(false)),
        ("baselineState", json!("absent")),
        ("suppressions", json!([{"kind":"unknown"}])),
        (
            "suppressions",
            json!([{"kind":"external","status":"unknown"}]),
        ),
    ] {
        let mut finding = issue();
        finding[key] = value;
        assert!(parse_value(&report(json!([finding]))).is_err());
    }
    let mut finding = issue();
    finding["kind"] = json!("pass");
    finding["level"] = json!("error");
    assert!(parse_value(&report(json!([finding]))).is_err());
}

#[test]
fn indices_cycles_foreign_uris_and_inconsistent_coordinates_are_rejected() {
    for location in [
        json!({"index":3}),
        json!({"index":-2}),
        json!({"index":"0"}),
        json!({}),
        json!({"uri":"src/a.rs","uriBaseId":"MISSING"}),
        json!({"uri":"https://example.invalid/source.rs"}),
        json!({"uri":"file://remote/source.rs"}),
        json!({"uri":"src/a.rs#part"}),
        json!({"uri":"src/a.rs?version=1"}),
        json!({"uri":"../escape.rs"}),
        json!({"uri":"src/a%ZZ.rs"}),
        json!({"uri":"src/a%.rs"}),
        json!({"uri":"src/%FF.rs"}),
        json!({"uri":"src/%00.rs"}),
        json!({"uri":"src/a%5Cb.rs"}),
        json!({"uri":"src\\a.rs"}),
        json!({"uri":"src/a\tb.rs"}),
        json!({"uri":"src/a.rs "}),
        json!({"uri":" src/a.rs"}),
    ] {
        let mut finding = issue();
        finding["locations"][0]["physicalLocation"]["artifactLocation"] = location.clone();
        assert!(
            parse_value(&report(json!([finding]))).is_err(),
            "{location}"
        );
    }
    let mut value = report(json!([issue()]));
    value["runs"][0]["results"][0]["locations"][0]["physicalLocation"]["artifactLocation"] =
        json!({"uri":"a.rs","uriBaseId":"ROOT"});
    value["runs"][0]["originalUriBaseIds"] = json!({"ROOT":{"uri":"src/","uriBaseId":"ROOT"}});
    assert!(parse_value(&value).is_err());
    for region in [
        json!({"startLine":0}),
        json!({"startLine":"2"}),
        json!({"startLine":4,"endLine":2}),
        json!({"endLine":4}),
    ] {
        let mut finding = issue();
        finding["locations"][0]["physicalLocation"]["region"] = region;
        assert!(parse_value(&report(json!([finding]))).is_err());
    }
    let mut value = report(json!([issue()]));
    value["runs"][0]["artifacts"] = json!([{"location":{"uri":"different.rs"}}]);
    value["runs"][0]["results"][0]["locations"][0]["physicalLocation"]["artifactLocation"]["index"] =
        json!(0);
    assert!(parse_value(&value).is_err());
    value["runs"][0]["artifacts"] = json!([{"location":{"index":0}}]);
    assert!(parse_value(&value).is_err());
}

#[test]
fn rule_and_logical_identity_references_cannot_disagree() {
    for value in [
        json!({"ruleId":"different","ruleIndex":0}),
        json!({"ruleId":"R","rule":{"id":"different"}}),
        json!({"ruleIndex":0,"rule":{"index":1}}),
        json!({"ruleIndex":8}),
        json!({"rule":{"index":0,"toolComponent":{"index":0}}}),
    ] {
        let mut finding = issue();
        for (key, value) in value.as_object().unwrap() {
            finding[key] = value.clone();
        }
        assert!(parse_value(&report(json!([finding]))).is_err());
    }
    let mut value = report(json!([issue()]));
    value["runs"][0]["logicalLocations"] = json!([{"fullyQualifiedName":"actual"}]);
    value["runs"][0]["results"][0]["locations"][0]["logicalLocations"] =
        json!([{"index":0,"fullyQualifiedName":"other"}]);
    assert!(parse_value(&value).is_err());
    value["runs"][0]["logicalLocations"] = json!([{"index":0}]);
    assert!(parse_value(&value).is_err());
    let mut value = report(json!([issue()]));
    value["runs"][0]["tool"]["driver"]["rules"] = json!([{"id":"R"},{"id":"R"}]);
    assert!(parse_value(&value).is_err());
}

#[test]
fn message_catalogs_and_expansion_are_bounded_and_not_silently_replaced() {
    let mut finding = issue();
    finding["message"] = json!({"id":"global","arguments":["value"]});
    let mut value = report(json!([finding]));
    value["runs"][0]["tool"]["driver"]["globalMessageStrings"] =
        json!({"global":{"text":"Use {0}","markdown":"Use **{0}**"}});
    assert_eq!(parse_value(&value).unwrap().issues[0].message, "Use value");
    for message in [
        json!({}),
        json!({"id":"missing"}),
        json!({"text":"{0}"}),
        json!({"text":"{oops}"}),
        json!({"text":"}"}),
        json!({"text":""}),
        json!({"markdown":"Missing required plain text"}),
        json!({"text":"{0}","arguments":[false]}),
    ] {
        let mut finding = issue();
        finding["message"] = message;
        assert!(parse_value(&report(json!([finding]))).is_err());
    }
    let mut finding = issue();
    finding["message"] = json!({"text":"{0}".repeat(3000),"arguments":["x".repeat(1024)]});
    assert!(parse_value(&report(json!([finding]))).is_err());
    let mut finding = issue();
    finding["locations"] = json!(vec![finding["locations"][0].clone(); 257]);
    assert!(parse_value(&report(json!([finding]))).is_err());
}

#[test]
fn location_resolution_budget_bounds_shared_references_and_expanded_paths() {
    let finding = issue();
    let value = report(json!([finding.clone()]));
    assert!(super::super::sarif_locations::locations(&value["runs"][0], &finding, &mut 0).is_err());
    let mut finding = finding;
    finding["locations"][0]["physicalLocation"]["artifactLocation"] =
        json!({"uri":"a".repeat(16 * 1024 + 1)});
    assert!(parse_value(&report(json!([finding.clone()]))).is_err());
    finding["locations"][0]["physicalLocation"]["artifactLocation"] =
        json!({"uri":"a".repeat(8000),"uriBaseId":"ROOT"});
    let mut value = report(json!([finding]));
    value["runs"][0]["originalUriBaseIds"] =
        json!({"ROOT":{"uri":format!("{}/", "b".repeat(9000))}});
    assert!(parse_value(&value).is_err());
    let mut finding = issue();
    finding["locations"][0]["logicalLocations"] =
        json!([{"fullyQualifiedName":"A | B"},{"fullyQualifiedName":"C"}]);
    let first = parse_value(&report(json!([finding.clone()])))
        .unwrap()
        .issues
        .remove(0);
    finding["locations"][0]["logicalLocations"] =
        json!([{"fullyQualifiedName":"A"},{"fullyQualifiedName":"B | C"}]);
    let second = parse_value(&report(json!([finding])))
        .unwrap()
        .issues
        .remove(0);
    assert_ne!(first.symbol, second.symbol);
    let mut finding = issue();
    finding["locations"][0]["physicalLocation"]["address"] = json!({"absoluteAddress":42});
    assert!(parse_value(&report(json!([finding]))).is_err());
    let mut finding = issue();
    finding["locations"] = json!([{}]);
    assert!(parse_value(&report(json!([finding]))).is_err());
}

#[test]
fn cumulative_expansion_and_resolution_limits_apply_to_the_whole_document() {
    let mut finding = issue();
    finding["message"] = json!({"text":"{0}".repeat(1800),"arguments":["x".repeat(1024)]});
    assert!(parse_value(&report(json!([finding.clone()]))).is_ok());
    let expanded = report(json!(vec![finding; 10]));
    assert!(
        parse_value(&expanded)
            .unwrap_err()
            .to_string()
            .contains("16 MiB")
    );
    let mut finding = issue();
    finding["locations"][0]["logicalLocations"] = json!(vec![json!({"index":0}); 32]);
    let mut value = report(json!(vec![finding; 210]));
    let mut chain: Vec<_> = (1..32).map(|index| json!({"index":index})).collect();
    chain.push(json!({"fullyQualifiedName":"fn"}));
    value["runs"][0]["logicalLocations"] = json!(chain);
    assert!(
        parse_value(&value)
            .unwrap_err()
            .to_string()
            .contains("work budget")
    );
}

#[test]
fn repeated_producer_and_rule_names_count_towards_expanded_evidence_budget() {
    let mut value = report(json!(vec![issue(); 20]));
    value["runs"][0]["tool"]["driver"]["name"] = json!("scan".repeat(300_000));
    assert!(
        parse_value(&value)
            .unwrap_err()
            .to_string()
            .contains("16 MiB")
    );
    let mut finding = issue();
    finding.as_object_mut().unwrap().remove("ruleId");
    finding["ruleIndex"] = json!(0);
    let mut value = report(json!(vec![finding; 20]));
    value["runs"][0]["tool"]["driver"]["rules"] = json!([{"id":"rule".repeat(300_000)}]);
    assert!(
        parse_value(&value)
            .unwrap_err()
            .to_string()
            .contains("16 MiB")
    );
}
