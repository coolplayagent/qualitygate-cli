//! Deterministic review subjects, independent of review assertions and the I/O layer.
use super::{
    Config, RuleSetting,
    catalog::{Catalog, Entry},
};
use crate::domain::SourceReviewEvidence;
use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub fn binding(id: &str, entry: &Entry, setting: &RuleSetting) -> Result<Option<String>> {
    if setting.source.is_none() && entry.custom.is_none() {
        return Ok(None);
    }
    let bytes = serde_json::to_vec(&serde_json::json!({
        "binding_version":1,"engine_version":env!("CARGO_PKG_VERSION"),
        "id":id,"definition":entry,"configuration":setting,
    }))?;
    Ok(Some(format!("sha256:{:x}", Sha256::digest(bytes))))
}

pub fn evidence(
    config: &Config,
    catalog: &Catalog,
) -> Result<BTreeMap<String, SourceReviewEvidence>> {
    let mut result = BTreeMap::new();
    for (id, setting) in &config.rules {
        let entry = catalog
            .entries
            .get(id)
            .with_context(|| format!("Unknown rule in source review: {id}"))?;
        if let Some(digest) = binding(id, entry, setting)? {
            result.insert(
                id.clone(),
                SourceReviewEvidence::new(digest, config.source_reviews.get(id).cloned()),
            );
        }
    }
    for id in config.source_reviews.keys() {
        if !result.contains_key(id) {
            bail!("Source review has no configured source-bound rule: {id}");
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn duplicate_review_keys_cannot_replace_an_earlier_record() {
        let digest = format!("sha256:{}", "1".repeat(64));
        let review = format!(
            "{{id: review, reviewer: team, reference: PR-1, resolution: initial_mapping, rationale: Reviewed, binding_digest: {digest}}}"
        );
        let yaml = format!(
            "schema_version: 1\nrules:\n  line-ending:\n    source: {{document: AGENTS.md, section: Rules, content_hash: {digest}}}\nsource_reviews:\n  line-ending: {review}\n  line-ending: {}\n",
            review.replace("PR-1", "PR-2")
        );
        assert!(super::super::parse(yaml.as_bytes()).is_err());
        let duplicate_rule = "schema_version: 1\nrules:\n  line-ending: {required: true}\n  line-ending: {required: false}\n";
        assert!(super::super::parse(duplicate_rule.as_bytes()).is_err());
        let nested = "schema_version: 1\nrules:\n  diff-size:\n    parameters: {max_added_lines: 1, max_added_lines: 100000}\n";
        assert!(super::super::parse(nested.as_bytes()).is_err());
    }
    #[test]
    fn binding_covers_effective_rule_settings_definition_and_source_without_review_cycles() {
        let config = super::super::parse(format!("schema_version: 1\nrules:\n  line-ending:\n    source: {{document: AGENTS.md, section: Rules, content_hash: sha256:{}}}\n", "1".repeat(64)).as_bytes()).unwrap();
        let catalog = Catalog::load(&config, std::iter::empty()).unwrap();
        let mut config = catalog.resolve(&config).unwrap();
        let initial = evidence(&config, &catalog).unwrap()["line-ending"]
            .expected_binding_digest
            .clone();
        let record = crate::domain::SourceReview {
            id: "review".into(),
            reviewer: "team".into(),
            reference: "PR-1".into(),
            resolution: crate::domain::ReviewResolution::InitialMapping,
            rationale: "Matches the source".into(),
            binding_digest: initial.clone(),
        };
        config.source_reviews.insert("line-ending".into(), record);
        assert_eq!(
            evidence(&config, &catalog).unwrap()["line-ending"].status,
            crate::domain::ReviewStatus::Bound
        );
        for key in ["required", "enabled", "severity", "depends_on", "source"] {
            let mut value = serde_json::to_value(&config.rules["line-ending"]).unwrap();
            value[key] = match key {
                "required" | "enabled" => serde_json::json!(false),
                "severity" => serde_json::json!("warning"),
                "depends_on" => serde_json::json!(["prepare"]),
                _ => {
                    serde_json::json!({"document":"AGENTS.md","section":"Rules","content_hash":format!("sha256:{}","2".repeat(64))})
                }
            };
            let setting = serde_json::from_value(value).unwrap();
            assert_ne!(
                binding("line-ending", &catalog.entries["line-ending"], &setting)
                    .unwrap()
                    .unwrap(),
                initial,
                "{key}"
            );
        }
        let mut changed = catalog.entries["line-ending"].clone();
        changed.builtin.as_mut().unwrap().version += 1;
        assert_ne!(
            binding("line-ending", &changed, &config.rules["line-ending"])
                .unwrap()
                .unwrap(),
            initial
        );
        assert!(
            evidence(
                &config,
                &Catalog {
                    entries: BTreeMap::new()
                }
            )
            .is_err()
        );
        config
            .source_reviews
            .get_mut("line-ending")
            .unwrap()
            .reviewer = "other".into();
        assert_eq!(
            evidence(&config, &catalog).unwrap()["line-ending"].expected_binding_digest,
            initial
        );
    }
}
