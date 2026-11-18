//! Rule execution consumes facts only from explicitly declared passing producers.

use super::execution_outcome::ProducedEvidence;
use crate::{
    config::{RuleSetting, catalog::Entry},
    domain::*,
    snapshot::Snapshot,
};
use std::collections::BTreeMap;
use std::sync::Arc;

pub(crate) struct Context {
    snapshot: Arc<Snapshot>,
    syntax: Arc<crate::adapters::syntax::Cache>,
    projects: BTreeMap<String, (usize, Arc<[ProjectFacts]>)>,
    pub(crate) tool_facts: BTreeMap<String, Arc<crate::tool_evidence::contracts::ToolFacts>>,
    pub(crate) coverage: BTreeMap<String, crate::tool_evidence::contracts::CoverageEvidence>,
    pub(crate) receipts: BTreeMap<String, super::fact_execution::Receipt>,
}

impl Context {
    pub(crate) fn new(snapshot: Arc<Snapshot>) -> Self {
        Self {
            snapshot,
            syntax: Arc::new(crate::adapters::syntax::Cache::default()),
            projects: BTreeMap::new(),
            tool_facts: BTreeMap::new(),
            coverage: BTreeMap::new(),
            receipts: BTreeMap::new(),
        }
    }

    fn project_groups(&self, dependencies: &[String]) -> Vec<(usize, Arc<[ProjectFacts]>)> {
        let mut groups: Vec<_> = dependencies
            .iter()
            .filter_map(|id| self.projects.get(id).cloned())
            .collect();
        // Preserve producer execution order for diagnostics and inventories.
        groups.sort_unstable_by_key(|(order, _)| *order);
        groups
    }

    pub(crate) fn publish(&mut self, id: &str, evidence: ProducedEvidence) {
        if let Some(receipt) = evidence.receipt {
            self.receipts.insert(id.into(), receipt);
        }
        if let Some(coverage) = evidence.coverage {
            self.coverage.insert(id.into(), coverage);
        }
        if !evidence.projects.is_empty() {
            self.projects
                .insert(id.into(), (self.projects.len(), evidence.projects.into()));
        }
        if let Some(facts) = evidence.tool_facts {
            self.tool_facts.insert(id.into(), Arc::new(facts));
        }
    }
}

pub(crate) async fn execute(
    id: &str,
    setting: &RuleSetting,
    entry: &Entry,
    context: &Context,
    provenance: Option<&crate::adapters::provenance::ProvenanceFacts>,
    git_input: Option<&crate::application::git_trailers::Input>,
) -> anyhow::Result<CheckResult> {
    let id = id.to_owned();
    let setting = setting.clone();
    let entry = entry.clone();
    let snapshot = Arc::clone(&context.snapshot);
    let syntax = Arc::clone(&context.syntax);
    let provenance = provenance.cloned();
    let git_input = git_input.cloned();
    let groups = context.project_groups(&setting.depends_on);
    let tool_facts = setting
        .depends_on
        .iter()
        .filter_map(|id| {
            context
                .tool_facts
                .get(id)
                .map(|facts| (id.clone(), Arc::clone(facts)))
        })
        .collect();
    tokio::task::spawn_blocking(move || {
        let projects: Vec<_> = groups.iter().flat_map(|(_, group)| group.iter()).collect();
        let git = if crate::adapters::git_trailers::needed(&setting, &entry) {
            match &git_input {
                Some(Ok(facts)) => Some(facts.as_ref()),
                other => {
                    let reason = other
                        .as_ref()
                        .and_then(|input| input.as_ref().err())
                        .map_or("Git history is unavailable", String::as_str);
                    let mut result = CheckResult::pending(&id, setting.required, setting.severity);
                    result.block(
                        ExecutionStatus::Blocked,
                        format!("Git declaration association could not complete: {reason}"),
                    );
                    return result;
                }
            }
        } else {
            None
        };
        let facts = crate::adapters::facts::RuleFacts {
            syntax: Some(&syntax),
            projects: &projects,
            provenance: provenance.as_ref(),
            git_trailers: git,
            tool_facts: Some(&tool_facts),
        };
        let mut result = if let Some(rule) = &entry.custom {
            crate::adapters::custom_rules::evaluate_borrowed(rule, &setting, &snapshot, &facts)
        } else {
            let builtin = entry.builtin.as_ref().expect("resolved builtin");
            let guidance = builtin.effective_diagnostic();
            crate::adapters::rules::evaluate_borrowed(
                &id,
                &builtin.implementation,
                &setting,
                &snapshot,
                &facts,
                guidance.as_deref(),
            )
        };
        result.rule_version = entry.version();
        result
            .metadata
            .insert("rule_definition".into(), serde_json::json!(entry));
        result.metadata.insert(
            "adapter_version".into(),
            serde_json::json!(env!("CARGO_PKG_VERSION")),
        );
        result
            .metadata
            .insert("depends_on".into(), serde_json::json!(setting.depends_on));
        for source in setting
            .source
            .iter()
            .chain(entry.custom.iter().map(|rule| &rule.source))
        {
            if let Err(error) = crate::application::policy::validate_source(source, &snapshot.files)
            {
                result.block(
                    ExecutionStatus::Blocked,
                    format!("Rule source requires review: {error:#}"),
                );
            }
        }
        result
    })
    .await
    .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config;
    use serde_json::json;

    #[test]
    fn project_dependencies_share_only_declared_groups_in_publication_order() {
        let root = tempfile::tempdir().unwrap();
        let mut ctx = Context::new(Arc::new(crate::application::test_support::snapshot(
            root.path(),
        )));
        for id in ["z", "unrelated", "a"] {
            let fact = serde_json::from_value(json!({"schema_version":1,"ecosystem":"maven","root":id,"manifest":"pom.xml","coordinate":id,"source_root":"src","test_source_root":"tests","declared":[],"resolved":[],"producer_check":id,"snapshot_digest":"fixture"})).unwrap();
            ctx.publish(
                id,
                ProducedEvidence {
                    projects: vec![fact],
                    ..Default::default()
                },
            );
        }
        let selected = ctx.project_groups(&["a".into(), "absent".into(), "z".into()]);
        assert_eq!(selected.len(), 2);
        assert!(Arc::ptr_eq(&selected[0].1, &ctx.projects["z"].1));
        assert!(Arc::ptr_eq(&selected[1].1, &ctx.projects["a"].1));
        assert!(ctx.project_groups(&[]).is_empty());
    }

    #[tokio::test]
    async fn rule_evaluation_ignores_untyped_display_metadata_even_from_declared_producers() {
        let root = tempfile::tempdir().unwrap();
        let mut context = Context::new(Arc::new(crate::application::test_support::snapshot(
            root.path(),
        )));
        let mut producer = CheckResult::pending("producer", true, Severity::Error);
        producer.complete();
        producer
            .metadata
            .insert("projects".into(), json!({"not":"project facts"}));
        let outcome = super::super::execution_outcome::ExecutionOutcome::new(
            producer,
            ProducedEvidence::default(),
        );
        context.publish(&outcome.check_result.id, outcome.evidence);
        assert!(context.project_groups(&["producer".into()]).is_empty());
        let config = config::parse(b"schema_version: 1\nrules: {line-ending: {}}\n").unwrap();
        let catalog = config::catalog::Catalog::load(&config, []).unwrap();
        let entry = &catalog.entries["line-ending"];
        let mut setting = entry.defaults();
        setting.depends_on = vec!["producer".into()];
        let result = execute("line-ending", &setting, entry, &context, None, None)
            .await
            .unwrap();
        assert_eq!(result.metadata["depends_on"], json!(["producer"]));
        assert_eq!(result.verdict, Some(Verdict::Pass));
    }

    #[tokio::test]
    async fn missing_git_association_and_stale_normative_source_block_an_otherwise_passing_rule() {
        let root = tempfile::tempdir().unwrap();
        let mut input = crate::application::test_support::snapshot(root.path());
        let mut source = crate::application::test_support::source(&mut input);
        source.content_hash = format!("sha256:{}", "0".repeat(64));
        let context = Context::new(Arc::new(input));
        let config = config::parse(b"schema_version: 1\nrules: {line-ending: {}}\n").unwrap();
        let catalog = config::catalog::Catalog::load(&config, []).unwrap();
        let entry = &catalog.entries["line-ending"];
        let mut setting = entry.defaults();
        setting
            .parameters
            .insert("marker".into(), json!({"type":"git_trailer"}));
        for unavailable in [None, Some(Err("shallow history".into()))] {
            let result = execute(
                "line-ending",
                &setting,
                entry,
                &context,
                None,
                unavailable.as_ref(),
            )
            .await
            .unwrap();
            assert_eq!(result.execution.status, ExecutionStatus::Blocked);
            assert!(
                result
                    .execution
                    .reason
                    .unwrap()
                    .contains(if unavailable.is_some() {
                        "shallow history"
                    } else {
                        "history is unavailable"
                    })
            );
            assert_eq!(result.verdict, None);
        }
        setting.parameters.clear();
        setting.source = Some(source);
        let result = execute("line-ending", &setting, entry, &context, None, None)
            .await
            .unwrap();
        assert_eq!(result.execution.status, ExecutionStatus::Blocked);
        assert!(
            result
                .execution
                .reason
                .unwrap()
                .contains("source requires review")
        );
        assert_eq!(result.verdict, None);
    }
}
