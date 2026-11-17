//! Config-time producer binding for closed V4 tool-fact source declarations.
use crate::config::{CommandCheck, CustomRule, RuleSetting};
use crate::rule_catalog::operator_v4::{FactSchema, SourceType};
use crate::tool_evidence::contracts::Scope;
use anyhow::{Context, Result, ensure};
use std::collections::BTreeMap;

pub fn source_types(rule: &CustomRule) -> Result<BTreeMap<String, SourceType>> {
    let mut sources: BTreeMap<_, _> = rule
        .sources
        .keys()
        .map(|name| (name.clone(), SourceType::Syntax))
        .collect();
    for (name, source) in &rule.fact_sources {
        source.validate().map_err(anyhow::Error::msg)?;
        ensure!(
            sources.insert(name.clone(), source.source_type()).is_none(),
            "Repeated source name: {name}"
        );
    }
    ensure!(sources.len() <= 16, "Expression exceeds 16 sources");
    Ok(sources)
}

pub fn validate_producers(
    rule: &CustomRule,
    setting: &RuleSetting,
    commands: &[CommandCheck],
) -> Result<()> {
    for source in rule.fact_sources.values() {
        source.validate().map_err(anyhow::Error::msg)?;
        ensure!(
            setting.depends_on.contains(&source.producer),
            "Tool source requires declared dependency: {}",
            source.producer
        );
        let producer = commands
            .iter()
            .find(|command| command.id == source.producer)
            .context("Tool source producer is unavailable")?;
        let output = producer
            .tool_facts
            .as_ref()
            .context("Tool source producer has no declared fact output")?;
        let scope = Scope {
            project: source.project.clone(),
            target: source.target.clone(),
            configuration: source.configuration.clone(),
        };
        ensure!(
            output.schema == source.schema && output.scope == scope,
            "Tool source output schema or scope differs from its producer"
        );
        if source.schema == FactSchema::FunctionMetric {
            ensure!(
                output
                    .metric_definitions()
                    .iter()
                    .any(|metric| Some(&metric.metric) == source.metric.as_ref()
                        && metric.unit == source.unit
                        && metric.measurement == source.measurement),
                "Tool source metric contract is not declared by its producer"
            );
        } else {
            ensure!(
                source.measurement == output.clone_measurement(),
                "Clone measurement contract differs from its producer"
            );
            ensure!(
                source.minimum_lines >= output.minimum_lines
                    && source.minimum_nodes >= output.minimum_nodes
                    && crate::tool_evidence::contracts::clone_floor_covers(
                        &source.measurement,
                        &output.similarity_floor,
                        &source.similarity_floor,
                    )?,
                "Clone source requests a broader universe than acquired by producer, including serialized-score boundaries"
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    fn rule_value() -> Value {
        json!({"schema_version":4,"id":"complexity-limit","version":1,"source":{"document":"AGENTS.md","section":"Tests","content_hash":format!("sha256:{}","0".repeat(64))},
            "requires_capabilities":["files"],"when":{"entity":"file","change":"all"},"fix":"Reduce complexity",
            "assertion":{"op":"all","input":{"op":"source","name":"metrics"},"predicate":{"op":"compare","left":{"op":"field","name":"value"},"relation":"le","right":{"op":"number","value":10}}},
            "fact_sources":{"metrics":{"producer":"scan","schema":"function_metric","schema_version":1,"project":"app","metric":"cc","unit":"count","measurement":"cc-v1"}}})
    }
    fn command() -> CommandCheck {
        serde_json::from_value(json!({"id":"scan","argv":["analyzer"],"tool_facts":{"adapter":"tool_facts","schema":"function_metric","path":"facts.json","tool_id":"analyzer","tool_version":"1","scope":{"project":"app"},"paths":["src/lib.rs"],"metrics":[{"metric":"cc","unit":"count","measurement":"cc-v1"}]}})).unwrap()
    }
    #[test]
    fn v4_schema_and_semantics_accept_sources_and_reject_legacy_or_untyped_extensions() {
        let value = rule_value();
        let parsed =
            crate::rule_catalog::rule_schema::parse(&serde_json::to_vec(&value).unwrap()).unwrap();
        assert_eq!(parsed.schema_version, 4);
        let example = crate::rule_catalog::rule_schema::parse(include_bytes!(
            "../../../skills/qualitygate-cli/references/examples/operators-v4.yaml"
        ))
        .unwrap();
        let reference = include_str!("../../../skills/qualitygate-cli/references/operators-v4.md");
        let section =
            crate::domain::normative::section(reference, &example.source.section).unwrap();
        assert_eq!(
            example.source.content_hash,
            crate::content_digest::sha256(section.as_bytes())
        );
        for edit in ["legacy", "field", "decimal", "namespace", "v3"] {
            let mut value = value.clone();
            match edit {
                "legacy" => value["expression"] = json!({"op":"bool","value":true}),
                "field" => value["assertion"]["predicate"]["left"]["name"] = json!("unregistered"),
                "decimal" => {
                    value["assertion"]["predicate"]["right"] =
                        json!({"op":"decimal","value":{"coefficient":"10","scale":1}})
                }
                "namespace" => {
                    value["sources"] = json!({"metrics":{"when":{"entity":"file","change":"all"},"requires_capabilities":["files"]}})
                }
                _ => value["schema_version"] = json!(3),
            }
            assert!(
                crate::rule_catalog::rule_schema::parse(&serde_json::to_vec(&value).unwrap())
                    .is_err(),
                "{edit}"
            );
        }
    }
    #[test]
    fn producers_require_explicit_dependencies_and_exact_scope_and_measurement() {
        let rule: CustomRule = serde_json::from_value(rule_value()).unwrap();
        let mut setting = RuleSetting::default();
        assert!(validate_producers(&rule, &setting, &[command()]).is_err());
        setting.depends_on.push("scan".into());
        validate_producers(&rule, &setting, &[command()]).unwrap();
        assert!(validate_producers(&rule, &setting, &[]).is_err());
        for edit in ["missing", "scope", "metric", "unit", "measurement"] {
            let mut command = command();
            let output = command.tool_facts.as_mut().unwrap();
            match edit {
                "scope" => output.scope.target = Some("//app:lib".into()),
                "metric" => output.metrics[0].metric = "other".into(),
                "unit" => output.metrics[0].unit = "percent".into(),
                "measurement" => output.metrics[0].measurement = "different".into(),
                _ => command.tool_facts = None,
            }
            assert!(
                validate_producers(&rule, &setting, &[command]).is_err(),
                "{edit}"
            );
        }
    }

    #[test]
    fn dryer_serialized_boundary_needs_a_lower_acquisition_floor() {
        use crate::tool_evidence::{contracts::DRYER_VERSION, decimal::Decimal};
        let measurement = format!("dryer@{DRYER_VERSION}/structural_jaccard_12dp");
        let mut value = rule_value();
        value["fact_sources"]["metrics"] = json!({"producer":"scan","schema":"clone_pair","schema_version":1,
            "project":"app","unit":"ratio","measurement":measurement,
            "similarity_floor":{"coefficient":"526315789474","scale":12}});
        value["assertion"] = json!({"op":"bool","value":true});
        let mut rule: CustomRule = serde_json::from_value(value).unwrap();
        let mut setting = RuleSetting::default();
        setting.depends_on.push("scan".into());
        let mut command = command();
        let output = command.tool_facts.as_mut().unwrap();
        output.schema = FactSchema::ClonePair;
        output.metrics.clear();
        output.clone_measurement = Some(measurement);
        for (floor, covers) in [
            ("0.526315789474", false),
            ("0.526315789473", true),
            ("0", true),
        ] {
            command.tool_facts.as_mut().unwrap().similarity_floor = Decimal::parse(floor).unwrap();
            assert_eq!(
                validate_producers(&rule, &setting, std::slice::from_ref(&command)).is_ok(),
                covers,
                "{floor}"
            );
        }
        // Other measurement definitions keep their exact declared-floor contract.
        rule.fact_sources.get_mut("metrics").unwrap().measurement = "generic-v1".into();
        let output = command.tool_facts.as_mut().unwrap();
        output.clone_measurement = Some("generic-v1".into());
        output.similarity_floor = Decimal::parse("0.526315789474").unwrap();
        validate_producers(&rule, &setting, &[command]).unwrap();
    }
}
