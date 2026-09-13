//! Explicit source-mapping review assertions for isolated test policies only.
use qualitygate::{
    config,
    domain::{ReviewResolution, SourceReview},
};
use std::{collections::BTreeMap, path::Path};

pub fn record(root: &Path) -> anyhow::Result<()> {
    let path = root.join("qualitygate.yaml");
    let bytes = std::fs::read(&path)?;
    let config = config::parse(&bytes)?;
    let catalog = config::catalog::read(root, &config)?;
    let config = catalog.resolve(&config)?;
    let mut records = BTreeMap::new();
    for (id, evidence) in config::source_reviews::evidence(&config, &catalog)? {
        records.insert(
            id.clone(),
            SourceReview {
                id: format!("fixture-review-{id}"),
                reviewer: "fixture-team".into(),
                reference: "isolated-test-policy".into(),
                resolution: ReviewResolution::InitialMapping,
                rationale: "The fixture's executable rule maps to its declared source section"
                    .into(),
                binding_digest: evidence.expected_binding_digest,
            },
        );
    }
    let mut value: serde_json::Value = serde_norway::from_slice(&bytes)?;
    value["source_reviews"] = serde_json::to_value(records)?;
    std::fs::write(path, serde_norway::to_string(&value)?)?;
    Ok(())
}
