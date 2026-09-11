//! Rule execution consumes facts only from explicitly declared passing producers.

use crate::{
    config::{RuleSetting, catalog::Entry},
    domain::*,
    snapshot::Snapshot,
};
use std::sync::Arc;

pub(super) async fn execute(
    id: &str,
    setting: &RuleSetting,
    entry: &Entry,
    snapshot: &Arc<Snapshot>,
    previous: &[CheckResult],
    provenance: Option<&crate::adapters::provenance::ProvenanceFacts>,
    git_input: Option<&super::git_trailers::Input>,
) -> anyhow::Result<CheckResult> {
    let id = id.to_owned();
    let setting = setting.clone();
    let entry = entry.clone();
    let snapshot = Arc::clone(snapshot);
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
            projects: &projects,
            provenance: provenance.as_ref(),
            git_trailers: git,
        };
        let mut result = if let Some(rule) = &entry.custom {
            crate::adapters::custom_rules::evaluate_with_context(rule, &setting, &snapshot, &facts)
        } else {
            let builtin = entry.builtin.as_ref().expect("resolved builtin");
            crate::adapters::rules::evaluate_with_context(
                &id,
                &builtin.implementation,
                &setting,
                &snapshot,
                &facts,
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
            if let Err(error) = super::policy::validate_source(source, &snapshot.files) {
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
