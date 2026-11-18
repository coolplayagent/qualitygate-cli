use super::*;
use crate::content_digest::sha256;
use serde_json::json;

fn sources() -> BTreeMap<String, Vec<u8>> {
    BTreeMap::from([(
        "src/lib.rs".into(),
        b"pub fn alpha() -> i32 {\n    1\n}\npub fn beta() -> i32 {\n    2\n}\n".to_vec(),
    )])
}

fn spec(adapter: ToolFormat) -> OutputSpec {
    serde_json::from_value(json!({
        "adapter":adapter,"schema":if adapter==ToolFormat::Crapper {"function_metric"} else {"clone_pair"},
        "path":".metrics/report.edn","tool_id":"tool","tool_version":if adapter==ToolFormat::Crapper {CRAPPER_VERSION} else {DRYER_VERSION},
        "scope":{"project":"demo"},"paths":["src/lib.rs"],"crate_name":"demo",
        "coverage_producer":if adapter==ToolFormat::Crapper {Some("coverage")} else {None},
    })).unwrap()
}

fn context(adapter: ToolFormat) -> NormalizeContext {
    let spec = spec(adapter);
    let inventory = super::super::inventory::expected_inventory(&spec, &sources()).unwrap();
    let argv = if adapter == ToolFormat::Crapper {
        vec!["crapper", "--use-existing-coverage", "src/lib.rs"]
    } else {
        vec![
            "dryer",
            "--threshold",
            "0",
            "--min-lines",
            "1",
            "--min-nodes",
            "0",
            "src/lib.rs",
        ]
    };
    let digest = sha256(b"snapshot");
    NormalizeContext {
        producer: "scan".into(),
        snapshot_digest: digest.clone(),
        artifact_digest: String::new(),
        inventory,
        coverage: Some(CoverageEvidence {
            producer: "coverage".into(),
            snapshot_digest: digest,
            artifact_digests: vec![sha256(b"coverage")],
            lines: BTreeMap::from([("src/lib.rs".into(), BTreeMap::from([(2, 1), (5, 0)]))]),
            inputs: BTreeMap::from([("coverage/lcov.info".into(), sha256(b"coverage"))]),
        }),
        execution: Some(ExecutionReceipt {
            argv: argv.into_iter().map(str::to_owned).collect(),
            exit_code: 0,
            stderr: "Wrote .metrics/report.edn\n".into(),
            tool_id: spec.tool_id.clone(),
            tool_version: spec.tool_version.clone(),
            scope_files: spec.paths.clone(),
            findings_exit: false,
        }),
        spec,
    }
}

fn run(raw: &str, context: &mut NormalizeContext) -> Result<ToolFacts> {
    context.artifact_digest = sha256(raw.as_bytes());
    normalize(raw.as_bytes(), context)
}

const CRAPPER: &str = r#"{:entries [{:name "alpha" :namespace "demo" :complexity 1 :coverage 100.0 :crap 1.0} {:name "beta" :namespace "demo" :complexity 1 :coverage 0.0 :crap 2.0}]}"#;
const DRYER: &str = r#"{:candidates [{:language "rust" :score 0.81 :left {:file "src/lib.rs" :start-line 1 :end-line 3} :right {:file "src/lib.rs" :start-line 4 :end-line 6} :left-nodes 22 :right-nodes 24}]}"#;

#[test]
fn native_metrics_keep_independent_coverage_and_observation_precision() {
    let mut context = context(ToolFormat::Crapper);
    let result = run(CRAPPER, &mut context).unwrap();
    assert!(result.complete);
    assert_eq!(result.records.len(), 6);
    assert_eq!(result.metric_definitions.len(), 3);
    let FactRecord::Metric(zero) = &result.records[4] else {
        panic!("expected coverage")
    };
    assert_eq!(zero.metric, "coverage");
    assert_eq!(zero.value, Decimal::from_i64(0));
    let unbound = normalize_unbound(CRAPPER.as_bytes(), &context).unwrap();
    assert!(!unbound.complete);
    assert_eq!(result.artifact_digest, unbound.artifact_digest);
    context
        .coverage
        .as_mut()
        .unwrap()
        .lines
        .remove("src/lib.rs");
    assert!(run(CRAPPER, &mut context).is_err());
}

#[test]
fn native_clone_pairs_keep_both_locations_filters_and_nodes() {
    let mut context = context(ToolFormat::Dryer);
    let result = run(DRYER, &mut context).unwrap();
    let FactRecord::ClonePair(pair) = &result.records[0] else {
        panic!("pair")
    };
    assert_eq!(pair.similarity, Decimal::parse("0.81").unwrap());
    assert_eq!(
        (
            pair.left.start_line,
            pair.right.start_line,
            pair.left_nodes,
            pair.right_nodes
        ),
        (1, 4, 22, 24)
    );
    let reversed = DRYER
        .replace(":start-line 1 :end-line 3", "REVERSE_ENDPOINT")
        .replace(":start-line 4 :end-line 6", ":start-line 1 :end-line 3")
        .replace("REVERSE_ENDPOINT", ":start-line 4 :end-line 6");
    let result = run(&reversed, &mut context).unwrap();
    let FactRecord::ClonePair(reversed) = &result.records[0] else {
        panic!("pair")
    };
    assert_eq!((reversed.left_nodes, reversed.right_nodes), (24, 22));
    assert!(run("{:candidates []}", &mut context).is_err());
    context.spec.similarity_floor = Decimal::parse("0.8").unwrap();
    context.execution.as_mut().unwrap().argv[2] = "0.8".into();
    assert!(run(DRYER, &mut context).is_ok());
    context.spec.similarity_floor = Decimal::parse("0.82").unwrap();
    context.execution.as_mut().unwrap().argv[2] = "0.82".into();
    assert!(run(DRYER, &mut context).is_err());
}

#[test]
fn incomplete_unknown_and_malformed_native_evidence_is_rejected() {
    for raw in [
        CRAPPER.replace("100.0", "nil"),
        CRAPPER.replace("100.0", "100.1"),
        CRAPPER.replace("2.0", "0.0"),
        CRAPPER.replace("1 :coverage", "0 :coverage"),
        CRAPPER.replace("demo", "missing"),
        CRAPPER.replace(":complexity", ":unknown"),
        CRAPPER.replace(":entries", ":extra 1 :entries"),
        "{:entries []}".into(),
    ] {
        assert!(
            run(&raw, &mut context(ToolFormat::Crapper)).is_err(),
            "{raw}"
        );
    }
    for raw in [
        DRYER.replace("rust", "python"),
        DRYER.replace("0.81", "1.01"),
        DRYER.replace("0.81", "0.1234567890123"),
        DRYER.replace(":left-nodes 22", ":left-nodes -1"),
        DRYER.replace(":end-line 3", ":end-line 99"),
        DRYER.replace(":score", ":extra 0 :score"),
    ] {
        assert!(run(&raw, &mut context(ToolFormat::Dryer)).is_err(), "{raw}");
    }
    let mut c = context(ToolFormat::Dryer);
    c.execution.as_mut().unwrap().stderr = "parser failed at file\n".into();
    assert!(run(DRYER, &mut c).is_err());
    c.execution.as_mut().unwrap().stderr.clear();
    c.execution.as_mut().unwrap().exit_code = 124;
    assert!(run(DRYER, &mut c).is_err());
    c.execution.as_mut().unwrap().exit_code = 0;
    c.execution.as_mut().unwrap().tool_version = "other".into();
    assert!(run(DRYER, &mut c).is_err());
}

#[test]
fn native_invocations_cannot_hide_selection_or_trigger_coverage_installation() {
    let c = context(ToolFormat::Crapper);
    for argv in [
        vec![],
        vec!["crapper", "src/lib.rs"],
        vec!["crapper", "--changed"],
        vec![
            "crapper",
            "--use-existing-coverage",
            "src/lib.rs",
            "src/lib.rs",
        ],
        vec![
            "crapper",
            "--use-existing-coverage",
            "--coverage-command",
            "install",
        ],
    ] {
        assert!(
            validate_invocation(
                &c.spec,
                &argv.into_iter().map(str::to_owned).collect::<Vec<_>>()
            )
            .is_err()
        );
    }
    let mut c = context(ToolFormat::Crapper);
    c.execution
        .as_mut()
        .unwrap()
        .argv
        .extend(["--threshold".into(), "1.5".into()]);
    c.execution.as_mut().unwrap().exit_code = 2;
    c.execution
        .as_mut()
        .unwrap()
        .stderr
        .push_str("CRAP threshold exceeded: 2 > 1.5\n");
    assert!(run(CRAPPER, &mut c).unwrap().complete);
    c.execution.as_mut().unwrap().exit_code = 0;
    c.execution.as_mut().unwrap().stderr.clear();
    assert!(run(CRAPPER, &mut c).is_err());
}

#[test]
fn generic_json_is_closed_complete_and_rebound_to_actual_artifact() {
    let mut c = context(ToolFormat::Crapper);
    let mut facts = run(CRAPPER, &mut c).unwrap();
    c.spec.adapter = ToolFormat::ToolFacts;
    c.spec.metrics = facts.metric_definitions.clone();
    c.spec.tool_id = "custom".into();
    c.spec.tool_version = "1".into();
    facts.tool_id = "custom".into();
    facts.tool_version = "1".into();
    facts.artifact_digest = "untrusted self claim".into();
    c.execution.as_mut().unwrap().tool_id = "custom".into();
    c.execution.as_mut().unwrap().tool_version = "1".into();
    c.execution.as_mut().unwrap().argv = vec!["custom".into()];
    let raw = serde_json::to_string(&facts).unwrap();
    assert_eq!(
        run(&raw, &mut c).unwrap().artifact_digest,
        sha256(raw.as_bytes())
    );
    c.execution.as_mut().unwrap().exit_code = 7;
    c.execution.as_mut().unwrap().findings_exit = true;
    assert!(run(&raw, &mut c).is_ok());
    let mut invalid = serde_json::to_value(&facts).unwrap();
    invalid["extra"] = json!(1);
    assert!(run(&invalid.to_string(), &mut c).is_err());
    for change in [
        "complete",
        "snapshot_digest",
        "producer",
        "schema_version",
        "records",
        "metric_definitions",
    ] {
        let mut invalid = serde_json::to_value(&facts).unwrap();
        invalid[change] = match change {
            "complete" => json!(false),
            "schema_version" => json!(2),
            "records" | "metric_definitions" => json!([]),
            _ => json!("mismatch"),
        };
        assert!(run(&invalid.to_string(), &mut c).is_err(), "{change}");
    }
}

#[test]
fn collection_identity_and_configuration_fail_closed() {
    let mut c = context(ToolFormat::Dryer);
    c.inventory.files.push("src/other.rs".into());
    assert!(run(DRYER, &mut c).is_err());
    let mut c = context(ToolFormat::Dryer);
    c.inventory.entities.push(c.inventory.entities[0].clone());
    assert!(run(DRYER, &mut c).is_err());
    let mut c = context(ToolFormat::Dryer);
    c.snapshot_digest = "bad".into();
    assert!(run(DRYER, &mut c).is_err());
    let mut c = context(ToolFormat::Dryer);
    c.artifact_digest = sha256(b"other");
    assert!(normalize(DRYER.as_bytes(), &c).is_err());
    let mut s = spec(ToolFormat::Dryer);
    s.scope.configuration = Some("debug".into());
    assert!(s.validate().is_err());
    s.scope.target = Some("//app:lib".into());
    assert!(s.validate().is_ok());
    s.paths.push("../outside.rs".into());
    assert!(s.validate().is_err());
    let mut s = spec(ToolFormat::Crapper);
    s.tool_version = "unknown".into();
    assert!(s.validate().is_err());
}

#[test]
fn generic_pair_inventory_and_record_semantics_cannot_be_silently_weakened() {
    let mut c = context(ToolFormat::Dryer);
    let facts = run(DRYER, &mut c).unwrap();
    c.spec.adapter = ToolFormat::ToolFacts;
    c.spec.clone_measurement = facts.clone_measurement.clone();
    let value = serde_json::to_value(&facts).unwrap();
    let normalized = run(&value.to_string(), &mut c).unwrap();
    assert!(jsonschema::is_valid(
        &super::super::schema::document(),
        &serde_json::to_value(normalized).unwrap(),
    ));
    let mut zero = value.clone();
    zero["minimum_lines"] = json!(0);
    c.spec.minimum_lines = 0;
    assert!(c.spec.validate().is_err());
    assert!(run(&zero.to_string(), &mut c).is_err());
    c.spec.minimum_lines = 1;
    for field in ["records", "inventory", "clone_measurement"] {
        let mut bad = value.clone();
        bad[field] = match field {
            "records" => json!([]),
            "inventory" => json!({"files":["src/lib.rs"],"entities":[]}),
            _ => json!("other_measurement"),
        };
        assert!(run(&bad.to_string(), &mut c).is_err(), "{field}");
    }
    for field in ["unit", "measurement", "id", "similarity", "left_nodes"] {
        let mut bad = value.clone();
        bad["records"][0][field] = match field {
            "id" => json!(""),
            "similarity" => json!({"coefficient":"10","scale":1}),
            "left_nodes" => json!(-1),
            _ => json!("invalid"),
        };
        assert!(run(&bad.to_string(), &mut c).is_err(), "{field}");
    }
    let mut duplicate = value.clone();
    duplicate["records"]
        .as_array_mut()
        .unwrap()
        .push(value["records"][0].clone());
    assert!(run(&duplicate.to_string(), &mut c).is_err());
}

#[test]
fn retention_checks_deadlines_and_bytes_before_publishing_records() {
    let mut c = context(ToolFormat::Dryer);
    let record = run(DRYER, &mut c).unwrap().records.remove(0);
    let mut expired = RecordBudget::new(Instant::now() - Duration::from_secs(1), &c).unwrap();
    let mut records = Vec::new();
    assert!(expired.retain(&mut records, record.clone()).is_err());
    assert!(records.is_empty());
    let mut exhausted = RecordBudget::new(Instant::now() + Duration::from_secs(1), &c).unwrap();
    exhausted.bytes = MAX_FACT_BYTES;
    assert!(exhausted.retain(&mut records, record).is_err());
    assert!(records.is_empty());
}
