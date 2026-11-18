//! Acquires only declared, completed producer collections before V4 evaluation.
use super::*;
use crate::rule_catalog::operator_v4::{FactSchema, SourceType};
use crate::rule_engine::operators_v4::{self, Collection, Record};
use crate::tool_evidence::contracts::{FactRecord, Scope, ToolFacts};
use anyhow::{Context, ensure};

pub(super) fn evaluate(
    rule: &CustomRule,
    snapshot: &Snapshot,
    facts: &crate::rule_engine::evaluation_facts::BorrowedRuleFacts<'_>,
    primary: &[Subject],
    result: &mut CheckResult,
    budget: &mut selection::Budget,
    output: &mut diagnostic_budget::Budget,
) -> Result<()> {
    budget.check()?;
    let mut collections = BTreeMap::new();
    collections.insert("entities".into(), syntax_collection(primary, budget)?);
    for (name, source) in &rule.sources {
        let selector = rule.for_source(source);
        requirements(&selector, snapshot, facts)?;
        let subjects = selection::collect(&selector, snapshot, facts, true, Some(budget))?;
        collections.insert(name.clone(), syntax_collection(&subjects, budget)?);
    }
    for (name, source) in &rule.fact_sources {
        budget.check()?;
        source.validate().map_err(anyhow::Error::msg)?;
        ensure!(
            !collections.contains_key(name),
            "Repeated source name: {name}"
        );
        let collection = facts
            .tool_facts
            .and_then(|facts| facts.get(&source.producer))
            .context("Declared tool fact producer is unavailable")?;
        let scope = Scope {
            project: source.project.clone(),
            target: source.target.clone(),
            configuration: source.configuration.clone(),
        };
        ensure!(
            collection.complete
                && collection.protocol_version == 1
                && collection.schema_version == source.schema_version,
            "Tool fact source is incomplete or unsupported"
        );
        collection.scope.validate()?;
        ensure!(
            collection.records.len() <= crate::domain::operator::MAX_ENTITIES
                && collection.metric_definitions.len() <= 64
                && collection.inventory.files.len() <= crate::domain::operator::MAX_ENTITIES
                && collection.inventory.entities.len() <= crate::domain::operator::MAX_ENTITIES,
            "Tool fact collection exceeds input inventory budget"
        );
        reserve_envelope(collection, budget)?;
        ensure!(
            collection.producer == source.producer
                && collection.snapshot_digest == snapshot.identity.content_digest,
            "Tool fact source producer/snapshot binding differs"
        );
        ensure!(
            collection.schema == source.schema && collection.scope == scope,
            "Tool fact source schema/scope differs"
        );
        if source.schema == FactSchema::FunctionMetric {
            ensure!(
                collection
                    .metric_definitions
                    .iter()
                    .any(|metric| Some(&metric.metric) == source.metric.as_ref()
                        && metric.unit == source.unit
                        && metric.measurement == source.measurement),
                "Required metric contract is unavailable"
            );
        } else {
            collection.similarity_floor.validate()?;
            ensure!(
                collection.minimum_lines > 0
                    && collection.similarity_floor
                        >= crate::tool_evidence::decimal::Decimal::from_i64(0)
                    && collection.similarity_floor
                        <= crate::tool_evidence::decimal::Decimal::from_i64(1),
                "Invalid acquired clone universe"
            );
            ensure!(
                collection.clone_measurement.as_ref() == Some(&source.measurement),
                "Clone measurement contract is unavailable"
            );
            ensure!(
                source.minimum_lines >= collection.minimum_lines
                    && source.minimum_nodes >= collection.minimum_nodes
                    && crate::tool_evidence::contracts::clone_floor_covers(
                        &source.measurement,
                        &collection.similarity_floor,
                        &source.similarity_floor,
                    )?,
                "Clone source requests a broader universe than acquired, including serialized-score boundaries"
            );
        }
        let mut records = Vec::new();
        for record in &collection.records {
            budget.reserve(0, Record::tool_bytes(record, &scope), 0)?;
            let selected = match record {
                FactRecord::Metric(metric) => {
                    ensure!(
                        source.schema == FactSchema::FunctionMetric,
                        "Unexpected metric in clone source"
                    );
                    metric.value.validate()?;
                    if metric.unit == "count" {
                        ensure!(
                            metric.value.as_i64()? >= 0,
                            "Count metric is not a nonnegative i64"
                        );
                    }
                    crate::tool_evidence::contracts::validate_location(&metric.location)?;
                    if source.metric.as_ref() != Some(&metric.metric) {
                        false
                    } else {
                        ensure!(
                            source.unit == metric.unit && source.measurement == metric.measurement,
                            "Metric units or measurement definition differ"
                        );
                        true
                    }
                }
                FactRecord::ClonePair(pair) => {
                    ensure!(
                        source.schema == FactSchema::ClonePair,
                        "Unexpected clone in metric source"
                    );
                    pair.similarity.validate()?;
                    ensure!(
                        pair.similarity >= crate::tool_evidence::decimal::Decimal::from_i64(0)
                            && pair.similarity
                                <= crate::tool_evidence::decimal::Decimal::from_i64(1)
                            && pair.left_nodes >= 0
                            && pair.right_nodes >= 0,
                        "Invalid clone similarity or node count"
                    );
                    crate::tool_evidence::contracts::validate_location(&pair.left)?;
                    crate::tool_evidence::contracts::validate_location(&pair.right)?;
                    ensure!(
                        source.unit == pair.unit && source.measurement == pair.measurement,
                        "Similarity units or measurement definition differ"
                    );
                    pair.similarity >= source.similarity_floor
                        && pair.left.end_line - pair.left.start_line + 1
                            >= source.minimum_lines as usize
                        && pair.right.end_line - pair.right.start_line + 1
                            >= source.minimum_lines as usize
                        && pair.left_nodes >= i64::from(source.minimum_nodes)
                        && pair.right_nodes >= i64::from(source.minimum_nodes)
                }
            };
            if selected {
                budget.reserve_bytes(Record::tool_bytes(record, &scope))?;
                let record = Record::tool(record, &scope);
                records.push(record);
            }
        }
        collections.insert(
            name.clone(),
            Collection {
                records,
                complete: true,
                source_type: source.source_type(),
            },
        );
    }
    budget.check()?;
    let assertion = rule.assertion.as_ref().context("Missing V4 assertion")?;
    let outcome = operators_v4::evaluate(assertion, &collections)?;
    if !outcome.passed {
        output.push(
            result,
            diagnostic(
                &rule.id,
                None,
                None,
                "Typed assertion did not hold for the complete declared collections".into(),
                serde_json::json!({"assertion":"v4","outcome":outcome}),
                &rule.fix,
                "assertion",
            ),
        )?;
    }
    result
        .metadata
        .insert("assertion".into(), serde_json::to_value(outcome)?);
    result.metadata.insert("assertion_inputs".into(),serde_json::json!({"snapshot":snapshot.identity,"sources":rule.sources,"fact_sources":rule.fact_sources,"complete":true}));
    Ok(())
}

// The input is shared, but each rule pays for every consumed envelope and record
// before retaining projections. Filtered-out records still consume input budget.
fn reserve_envelope(collection: &ToolFacts, budget: &mut selection::Budget) -> Result<()> {
    budget.reserve_bytes(std::mem::size_of::<ToolFacts>())?;
    for value in [
        &collection.producer,
        &collection.snapshot_digest,
        &collection.tool_id,
        &collection.tool_version,
        &collection.artifact_digest,
        &collection.scope.project,
        &collection.similarity_floor.coefficient,
    ]
    .into_iter()
    .chain(collection.scope.target.iter())
    .chain(collection.scope.configuration.iter())
    .chain(collection.clone_measurement.iter())
    {
        budget.reserve_bytes(value.len())?;
    }
    for metric in &collection.metric_definitions {
        budget.reserve_bytes(
            96usize
                .saturating_add(metric.metric.len())
                .saturating_add(metric.unit.len())
                .saturating_add(metric.measurement.len()),
        )?;
    }
    for path in &collection.inventory.files {
        budget.reserve_bytes(32usize.saturating_add(path.len()))?;
    }
    for entity in &collection.inventory.entities {
        budget.reserve_bytes(
            128usize
                .saturating_add(entity.namespace.len())
                .saturating_add(entity.name.len())
                .saturating_add(entity.location.file.len())
                .saturating_add(entity.location.symbol.len()),
        )?;
    }
    Ok(())
}
fn syntax_collection(subjects: &[Subject], budget: &mut selection::Budget) -> Result<Collection> {
    let mut records = Vec::new();
    for subject in subjects.iter().filter(|subject| subject.triggered) {
        // Subjects remain alive while evaluating their owned typed projection.
        // Include field-map overhead and the possible duplicate location strings.
        let bytes = 1024usize
            .saturating_add(subject.name.len().saturating_mul(2))
            .saturating_add(subject.text.len())
            .saturating_add(
                subject
                    .file
                    .as_ref()
                    .map_or(0, String::len)
                    .saturating_mul(2),
            );
        budget.reserve_bytes(bytes)?;
        records.push(Record::syntax(
            &subject.name,
            &subject.text,
            subject.file.as_deref(),
            subject.range.as_ref(),
        ));
    }
    Ok(Collection {
        records,
        complete: true,
        source_type: SourceType::Syntax,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool_evidence::decimal::Decimal;
    use serde_json::json;
    use std::sync::Arc;

    fn snapshot() -> Snapshot {
        let files = BTreeMap::from([(
            "src/lib.rs".into(),
            crate::snapshot::File {
                bytes: b"fn left() {\n}\nfn right() {\n}\n".to_vec(),
                executable: false,
            },
        )]);
        Snapshot {
            scope_evidence: Default::default(),
            root: "/repo".into(),
            identity: crate::snapshot::Identity {
                verification_digest: None,
                mode: "worktree".into(),
                base: "base".into(),
                head: "head".into(),
                content_digest: "snapshot".into(),
                merge_request: None,
            },
            changes: crate::snapshot::compare_files(&BTreeMap::new(), &files),
            files,
            base_files: BTreeMap::new(),
            path_filter: None,
            commits: vec![],
        }
    }
    fn rule() -> CustomRule {
        serde_json::from_value(json!({"schema_version":4,"id":"clone-limit","version":1,
            "source":{"document":"AGENTS.md","section":"Clones","content_hash":format!("sha256:{}","0".repeat(64))},
            "requires_capabilities":["files"],"when":{"entity":"file","change":"all"},"fix":"Remove clone",
            "fact_sources":{"pairs":{"producer":"scan","schema":"clone_pair","schema_version":1,"project":"app",
                "unit":"ratio","measurement":"clone-v1","similarity_floor":{"coefficient":"8","scale":1}}},
            "assertion":{"op":"all","input":{"op":"source","name":"pairs"},"as":"pair",
                "predicate":{"op":"compare","left":{"op":"field","name":"similarity"},"relation":"lt",
                    "right":{"op":"decimal","value":{"coefficient":"8","scale":1}}}}})).unwrap()
    }
    fn collection() -> ToolFacts {
        let left = json!({"file":"src/lib.rs","start_line":1,"end_line":2,"symbol":"app::left"});
        let right = json!({"file":"src/lib.rs","start_line":3,"end_line":4,"symbol":"app::right"});
        serde_json::from_value(
            json!({"protocol_version":1,"schema":"clone_pair","schema_version":1,
            "producer":"scan","snapshot_digest":"snapshot","tool_id":"fixture","tool_version":"1",
            "artifact_digest":"artifact","scope":{"project":"app"},"complete":true,
            "inventory":{"files":["src/lib.rs"],"entities":[
                {"namespace":"app","name":"left","location":left},
                {"namespace":"app","name":"right","location":right}]},
            "clone_measurement":"clone-v1","records":[{"kind":"clone_pair","id":"pair",
                "left":left,"right":right,"similarity":{"coefficient":"81","scale":2},
                "unit":"ratio","measurement":"clone-v1","left_nodes":5,"right_nodes":5}]}),
        )
        .unwrap()
    }
    fn run(rule: &CustomRule, collection: Option<ToolFacts>) -> CheckResult {
        let inputs = collection.map(|facts| BTreeMap::from([("scan".into(), Arc::new(facts))]));
        super::super::evaluate_with_context(
            rule,
            &RuleSetting::default(),
            &snapshot(),
            &crate::rule_engine::evaluation_facts::RuleFacts {
                tool_facts: inputs.as_ref(),
                ..Default::default()
            },
        )
    }
    #[test]
    fn clone_source_filters_preserve_both_locations_and_refuse_broader_acquisition_claims() {
        let rule = rule();
        let result = run(&rule, Some(collection()));
        assert_eq!(result.verdict, Some(Verdict::Fail), "{result:?}");
        assert_eq!(result.metadata["assertion"]["sources"]["pairs"], 1);
        let observations = result.metadata["assertion"]["false_observations"]
            .as_array()
            .unwrap();
        let located = observations
            .iter()
            .find(|observation| observation["bindings"]["pair"].is_object())
            .unwrap();
        assert_eq!(
            located["bindings"]["pair"]["related"][0]["symbol"],
            "app::left"
        );
        assert_eq!(
            located["bindings"]["pair"]["related"][1]["symbol"],
            "app::right"
        );

        for filter in ["floor", "lines", "nodes"] {
            let mut acquired = collection();
            match filter {
                "floor" => acquired.similarity_floor = Decimal::parse("0.82").unwrap(),
                "lines" => acquired.minimum_lines = 3,
                _ => acquired.minimum_nodes = 6,
            }
            let result = run(&rule, Some(acquired));
            assert_eq!(
                result.execution.status,
                ExecutionStatus::Blocked,
                "{filter}: {result:?}"
            );
            assert!(result.verdict.is_none());
        }
        let mut restricted = rule.clone();
        restricted
            .fact_sources
            .get_mut("pairs")
            .unwrap()
            .similarity_floor = Decimal::parse("0.82").unwrap();
        let empty = run(&restricted, Some(collection()));
        assert_eq!(empty.verdict, Some(Verdict::Pass));
        assert_eq!(empty.metadata["assertion"]["sources"]["pairs"], 0);
        restricted.assertion = Some(serde_json::from_value(json!({"op":"compare", "left":{"op":"count","input":{"op":"source","name":"pairs"}},"relation":"ge","right":{"op":"number","value":1}})).unwrap());
        assert_eq!(
            run(&restricted, Some(collection())).verdict,
            Some(Verdict::Fail)
        );
    }
    #[test]
    fn missing_incomplete_malformed_and_misbound_sources_block_before_assertion_short_circuit() {
        let mut rule = rule();
        rule.assertion = Some(serde_json::from_value(json!({"op":"bool","value":true})).unwrap());
        assert_eq!(run(&rule, None).execution.status, ExecutionStatus::Blocked);
        for fault in [
            "incomplete",
            "protocol",
            "version",
            "snapshot",
            "producer",
            "scope",
            "measurement",
            "range",
            "precision",
            "similarity",
            "nodes",
            "universe",
        ] {
            let mut facts = collection();
            match fault {
                "incomplete" => facts.complete = false,
                "protocol" => facts.protocol_version = 2,
                "version" => facts.schema_version = 2,
                "snapshot" => facts.snapshot_digest = "old".into(),
                "producer" => facts.producer = "other".into(),
                "scope" => facts.scope.target = Some("//app:lib".into()),
                "measurement" => facts.clone_measurement = None,
                "universe" => facts.similarity_floor.scale = 19,
                _ => {
                    let FactRecord::ClonePair(pair) = &mut facts.records[0] else {
                        unreachable!()
                    };
                    match fault {
                        "range" => pair.left.start_line = 0,
                        "precision" => pair.similarity.scale = 19,
                        "nodes" => pair.left_nodes = -1,
                        _ => pair.similarity = Decimal::from_i64(2),
                    }
                }
            }
            let result = run(&rule, Some(facts));
            assert_eq!(
                result.execution.status,
                ExecutionStatus::Blocked,
                "{fault}: {result:?}"
            );
            assert!(result.verdict.is_none());
        }
    }
    #[test]
    fn shared_envelopes_and_filtered_records_consume_each_rules_input_budget() {
        let mut rule = rule();
        rule.fact_sources.get_mut("pairs").unwrap().similarity_floor =
            Decimal::parse("0.9").unwrap();
        let mut facts = collection();
        facts.inventory.files[0] = "x".repeat(crate::domain::operator::MAX_FACT_BYTES);
        assert_eq!(
            run(&rule, Some(facts)).execution.status,
            ExecutionStatus::Blocked
        );
        let mut facts = collection();
        facts.records = vec![facts.records[0].clone(); crate::domain::operator::MAX_ENTITIES];
        // Every pair would be filtered out; acquisition still cannot retain this input.
        assert_eq!(
            run(&rule, Some(facts)).execution.status,
            ExecutionStatus::Blocked
        );
    }

    #[test]
    fn syntax_projection_is_reserved_while_original_subjects_are_retained() {
        let mut subject = Subject {
            name: "example".into(),
            text: "x".repeat(1024),
            file: Some("src/lib.rs".into()),
            range: None,
            identity: "example".into(),
            declaration: None,
            triggered: true,
        };
        let mut budget = selection::Budget::new();
        budget
            .reserve(
                subject.name.len(),
                subject.text.len(),
                subject.file.as_ref().unwrap().len(),
            )
            .unwrap();
        let projected = syntax_collection(std::slice::from_ref(&subject), &mut budget).unwrap();
        assert_eq!(projected.records.len(), 1);
        subject.text = "x".repeat(crate::domain::operator::MAX_FACT_BYTES / 2);
        let mut budget = selection::Budget::new();
        budget
            .reserve(
                subject.name.len(),
                subject.text.len(),
                subject.file.as_ref().unwrap().len(),
            )
            .unwrap();
        assert!(
            syntax_collection(&[subject], &mut budget)
                .err()
                .unwrap()
                .to_string()
                .contains("32 MiB")
        );
    }

    #[test]
    fn same_file_in_distinct_target_configurations_retains_each_scope_and_requires_both() {
        let mut rule = rule();
        let template = rule.fact_sources.remove("pairs").unwrap();
        let mut inputs = BTreeMap::new();
        let mut assertions = Vec::new();
        for (name, configuration, value) in [("debug", "dbg", "0.81"), ("release", "opt", "0.9")] {
            let producer = format!("scan_{name}");
            let target = format!("//app:{name}");
            let mut source = template.clone();
            source.producer = producer.clone();
            source.target = Some(target.clone());
            source.configuration = Some(configuration.into());
            rule.fact_sources.insert(name.into(), source);
            let mut facts = collection();
            facts.producer = producer.clone();
            facts.scope.target = Some(target.clone());
            facts.scope.configuration = Some(configuration.into());
            let FactRecord::ClonePair(pair) = &mut facts.records[0] else {
                unreachable!()
            };
            pair.similarity = Decimal::parse(value).unwrap();
            inputs.insert(producer, Arc::new(facts));
            assertions.push(
                json!({"op":"compare","left":{"op":"count","input":{"op":"source","name":name}},
                "relation":"eq","right":{"op":"number","value":1}}),
            );
            assertions.push(json!({"op":"all","input":{"op":"source","name":name},"predicate":{"op":"and","args":[
                {"op":"compare","left":{"op":"field","name":"target"},"relation":"eq","right":{"op":"string","value":target}},
                {"op":"compare","left":{"op":"field","name":"configuration"},"relation":"eq","right":{"op":"string","value":configuration}},
                {"op":"compare","left":{"op":"field","name":"similarity"},"relation":"eq","right":{"op":"decimal","value":Decimal::parse(value).unwrap()}}
            ]}}));
        }
        rule.assertion =
            Some(serde_json::from_value(json!({"op":"and","args":assertions})).unwrap());
        let evaluate = |inputs: &BTreeMap<_, _>| {
            super::super::evaluate_with_context(
                &rule,
                &RuleSetting::default(),
                &snapshot(),
                &crate::rule_engine::evaluation_facts::RuleFacts {
                    tool_facts: Some(inputs),
                    ..Default::default()
                },
            )
        };
        // Both collections deliberately reuse the same record ID and locations.
        let result = evaluate(&inputs);
        assert_eq!(result.verdict, Some(Verdict::Pass), "{result:?}");
        assert_eq!(result.metadata["assertion"]["sources"]["debug"], 1);
        assert_eq!(result.metadata["assertion"]["sources"]["release"], 1);
        let release = inputs.remove("scan_release").unwrap();
        let missing = evaluate(&inputs);
        assert_eq!(missing.execution.status, ExecutionStatus::Blocked);
        assert!(missing.verdict.is_none());
        inputs.insert("scan_release".into(), release);
        Arc::make_mut(inputs.get_mut("scan_release").unwrap())
            .scope
            .configuration = Some("dbg".into());
        let mismatched = evaluate(&inputs);
        assert_eq!(mismatched.execution.status, ExecutionStatus::Blocked);
        assert!(mismatched.verdict.is_none());
    }

    #[test]
    fn dryer_serialized_boundary_cannot_turn_filtered_omissions_into_empty_passes() {
        use crate::tool_evidence::contracts::DRYER_VERSION;
        let measurement = format!("dryer@{DRYER_VERSION}/structural_jaccard_12dp");
        let boundary = Decimal::parse("0.526315789474").unwrap();
        let mut rule = rule();
        let source = rule.fact_sources.get_mut("pairs").unwrap();
        source.measurement = measurement.clone();
        source.similarity_floor = boundary.clone();
        rule.assertion = Some(
            serde_json::from_value(json!({"op":"compare",
            "left":{"op":"count","input":{"op":"source","name":"pairs"}},"relation":"eq",
            "right":{"op":"number","value":0}}))
            .unwrap(),
        );
        let mut omitted = collection();
        omitted.clone_measurement = Some(measurement.clone());
        omitted.similarity_floor = boundary.clone();
        omitted.records.clear();
        let result = run(&rule, Some(omitted));
        assert_eq!(result.execution.status, ExecutionStatus::Blocked);
        assert!(result.verdict.is_none());
        for floor in ["0.526315789473", "0"] {
            let mut complete = collection();
            complete.clone_measurement = Some(measurement.clone());
            complete.similarity_floor = Decimal::parse(floor).unwrap();
            let FactRecord::ClonePair(pair) = &mut complete.records[0] else {
                unreachable!()
            };
            pair.measurement = measurement.clone();
            pair.similarity = boundary.clone();
            let result = run(&rule, Some(complete));
            assert_eq!(result.verdict, Some(Verdict::Fail), "{floor}: {result:?}");
            assert_eq!(result.metadata["assertion"]["sources"]["pairs"], 1);
        }
    }
}
