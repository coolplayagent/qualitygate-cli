//! Caller-controlled authorization for external acceptance signers.

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrustStore {
    pub schema_version: u32,
    pub repository: String,
    pub keys: Vec<TrustedKey>,
    #[serde(default)]
    pub revoked_records: Vec<String>,
    pub max_age_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrustedKey {
    pub id: String,
    pub public_key: String,
    #[serde(default)]
    pub checks: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub provenance_rules: Vec<String>,
    #[serde(default)]
    pub tasks: Vec<String>,
    #[serde(default)]
    pub allow_repository_checks: bool,
}

pub fn parse(bytes: &[u8]) -> Result<TrustStore> {
    if bytes.len() > 256 * 1024 {
        bail!("Trust store exceeds 256 KiB");
    }
    let store: TrustStore = serde_json::from_slice(bytes)?;
    if store.schema_version != 1
        || store.repository.trim().is_empty()
        || store.repository.len() > 2048
        || !(1..=64).contains(&store.keys.len())
        || !(1..=31_536_000).contains(&store.max_age_seconds)
    {
        bail!(
            "Trust store requires version 1, repository, 1..64 keys and a maximum age of 1..31536000 seconds"
        );
    }
    let mut ids = BTreeSet::new();
    for key in &store.keys {
        super::validation::validate_id(&key.id)?;
        if !ids.insert(&key.id)
            || key.checks.is_empty() && key.provenance_rules.is_empty()
            || key.public_key.len() > 128
        {
            bail!(
                "Trusted keys need unique IDs, bounded public keys and explicit check or provenance-rule authorization"
            );
        }
        identifiers(&key.checks)?;
        identifiers(&key.provenance_rules)?;
        identifiers(&key.tasks)?;
    }
    identifiers(&store.revoked_records)?;
    Ok(store)
}

fn identifiers(values: &[String]) -> Result<()> {
    if values.len() > 1024 {
        bail!("Trust store identifier list exceeds 1024 entries");
    }
    let mut unique = BTreeSet::new();
    for value in values {
        if value.trim().is_empty() || value.len() > 256 || !unique.insert(value) {
            bail!("Trust store identifiers must be nonempty, unique and at most 256 bytes");
        }
    }
    Ok(())
}
