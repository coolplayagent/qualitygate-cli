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
) -> anyhow::Result<CheckResult> {
    let id = id.to_owned();
    let setting = setting.clone();
    let entry = entry.clone();
    let snapshot = Arc::clone(snapshot);
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
        let mut result = if let Some(rule) = &entry.custom {
            crate::adapters::custom_rules::evaluate_with_projects(
                rule, &setting, &snapshot, &projects,
            )
        } else {
            let builtin = entry.builtin.as_ref().expect("resolved builtin");
            crate::adapters::rules::evaluate_with_projects(
                &id,
                &builtin.implementation,
                &setting,
                &snapshot,
                &projects,
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
