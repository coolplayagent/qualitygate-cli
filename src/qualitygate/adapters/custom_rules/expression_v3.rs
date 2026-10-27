//! Named sources reuse the existing selector adapters without top-level path clipping.
use super::*;
use crate::{
    adapters::{
        operators::Entity,
        operators_v3::{self, Collection},
    },
    domain::operator::{MAX_ENTITIES, MAX_FACT_BYTES},
};
use std::time::{Duration, Instant};

pub(super) fn evaluate(
    rule: &CustomRule,
    snapshot: &Snapshot,
    facts: &super::super::facts::RuleFacts<'_>,
    primary: &[Subject],
    result: &mut CheckResult,
) -> Result<()> {
    let started = Instant::now();
    let mut count = 0usize;
    let mut bytes = 0usize;
    let mut budget = |subjects: &[Subject]| -> Result<()> {
        for subject in subjects.iter().filter(|s| s.triggered) {
            count = count.saturating_add(1);
            bytes = bytes
                .saturating_add(subject.name.len())
                .saturating_add(subject.text.len())
                .saturating_add(subject.file.as_ref().map_or(0, String::len));
        }
        anyhow::ensure!(
            count <= MAX_ENTITIES && bytes <= MAX_FACT_BYTES,
            "Named sources exceed 50000 entities or 32 MiB facts"
        );
        anyhow::ensure!(
            started.elapsed() < Duration::from_secs(30),
            "Named source acquisition exceeded its 30-second deadline"
        );
        Ok(())
    };
    budget(primary)?;
    let mut owned = BTreeMap::new();
    for (name, source) in &rule.sources {
        let selector = rule.for_source(source);
        requirements(&selector, snapshot, facts)?;
        let subjects = selected_subjects(&selector, snapshot, facts, true)?;
        budget(&subjects)?;
        owned.insert(name.clone(), subjects);
    }
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
