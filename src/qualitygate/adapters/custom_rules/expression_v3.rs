//! Named sources reuse the existing selector adapters without top-level path clipping.
use super::*;
use crate::adapters::{
    operators::Entity,
    operators_v3::{self, Collection},
};

pub(super) fn evaluate(
    rule: &CustomRule,
    snapshot: &Snapshot,
    facts: &super::super::facts::RuleFacts<'_>,
    primary: &[Subject],
    result: &mut CheckResult,
    budget: &mut selection::Budget,
) -> Result<()> {
    budget.check()?;
    let mut owned = BTreeMap::new();
    for (name, source) in &rule.sources {
        budget.check()?;
        let selector = rule.for_source(source);
        requirements(&selector, snapshot, facts)?;
        let subjects = selection::collect(&selector, snapshot, facts, true, Some(budget))?;
        owned.insert(name.clone(), subjects);
    }
    budget.check()?;
    let mut collections = BTreeMap::new();
    for (name, subjects) in std::iter::once(("entities", primary)).chain(
        owned
            .iter()
            .map(|(name, subjects)| (name.as_str(), subjects.as_slice())),
    ) {
        collections.insert(
            name.to_owned(),
            Collection {
                complete: true,
                entities: subjects
                    .iter()
                    .filter(|s| s.triggered)
                    .map(|s| Entity {
                        name: &s.name,
                        text: &s.text,
                        path: s.file.as_deref(),
                        range: s.range.as_ref(),
                    })
                    .collect(),
            },
        );
    }
    let expression = rule
        .expression
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Missing v3 expression"))?;
    let outcome = operators_v3::evaluate(expression, &collections)?;
    if !outcome.passed {
        result.diagnostics.push(diagnostic(
            &rule.id,
            None,
            None,
            "Typed expression did not hold for the complete named collections".into(),
            serde_json::json!({"assertion":"expression","outcome":outcome}),
            &rule.fix,
            "expression",
        ));
    }
    result
        .metadata
        .insert("expression".into(), serde_json::to_value(outcome)?);
    result.metadata.insert(
        "expression_inputs".into(),
        serde_json::json!({"snapshot":snapshot.identity,"sources":rule.sources,"complete":true}),
    );
    Ok(())
}
