//! Rule execution consumes facts only from explicitly declared passing producers.

use crate::{
    config::{RuleSetting, catalog::Entry},
    domain::*,
    snapshot::Snapshot,
};
use std::sync::Arc;

pub(crate) struct Context {
    snapshot: Arc<Snapshot>,
    syntax: Arc<crate::adapters::syntax::Cache>,
}

impl Context {
    pub(crate) fn new(snapshot: Arc<Snapshot>) -> Self {
        Self {
            snapshot,
            syntax: Arc::new(crate::adapters::syntax::Cache::default()),
        }
    }
}

pub(crate) async fn execute(
    id: &str,
    setting: &RuleSetting,
    entry: &Entry,
    context: &Context,
    previous: &[CheckResult],
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
    let mut projects = Vec::new();
    for producer in previous
        .iter()
        .filter(|result| setting.depends_on.contains(&result.id))
    {
        if let Some(value) = producer.metadata.get("projects") {
            projects.extend(serde_json::from_value::<Vec<ProjectFacts>>(value.clone())?);
        }
    }
    tokio::task::spawn_blocking(move || {
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
        };
        let mut result = if let Some(rule) = &entry.custom {
            crate::adapters::custom_rules::evaluate_with_context(rule, &setting, &snapshot, &facts)
        } else {
            let builtin = entry.builtin.as_ref().expect("resolved builtin");
            let guidance = builtin.effective_diagnostic();
            crate::adapters::rules::evaluate_with_guidance(
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

    #[tokio::test]
    async fn rule_evaluation_consumes_only_declared_project_producers_and_rejects_malformed_facts()
    {
        let root = tempfile::tempdir().unwrap();
        let context = Context::new(Arc::new(crate::application::test_support::snapshot(
            root.path(),
        )));
        let config = config::parse(b"schema_version: 1\nrules: {line-ending: {}}\n").unwrap();
        let catalog = config::catalog::Catalog::load(&config, []).unwrap();
        let entry = &catalog.entries["line-ending"];
        let mut setting = entry.defaults();
        let mut producer = CheckResult::pending("producer", true, Severity::Error);
        producer.complete();
        producer
            .metadata
            .insert("projects".into(), json!({"not":"project facts"}));
        let ignored = execute(
            "line-ending",
            &setting,
            entry,
            &context,
            std::slice::from_ref(&producer),
            None,
            None,
        )
        .await
        .unwrap();
        assert_eq!(ignored.execution.status, ExecutionStatus::Completed);
        setting.depends_on = vec!["producer".into()];
        assert!(
            execute(
                "line-ending",
                &setting,
                entry,
                &context,
                std::slice::from_ref(&producer),
                None,
                None
            )
            .await
            .is_err()
        );
        producer.metadata.insert("projects".into(), json!([]));
        let result = execute(
            "line-ending",
            &setting,
            entry,
            &context,
            &[producer],
            None,
            None,
        )
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
                &[],
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
        let result = execute("line-ending", &setting, entry, &context, &[], None, None)
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
