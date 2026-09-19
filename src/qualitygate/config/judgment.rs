//! Strict, bounded loading for optional external judgment policy.

use crate::domain::judgment::{JudgmentPolicy, Question};
use anyhow::{Result, bail};

pub const MAX_POLICY_BYTES: usize = 64 * 1024;

pub fn parse(bytes: &[u8]) -> Result<JudgmentPolicy> {
    if bytes.len() > MAX_POLICY_BYTES {
        bail!("Judgment policy exceeds 64 KiB");
    }
    let policy: JudgmentPolicy = super::parse_yaml(bytes)?;
    policy.validate()?;
    Ok(policy)
}

pub fn question_digest(bytes: &[u8]) -> Result<String> {
    if bytes.len() > MAX_POLICY_BYTES {
        bail!("Question exceeds 64 KiB");
    }
    let mut question: Question = super::parse_yaml(bytes)?;
    question.source_digest = question.digest()?;
    question.validate()?;
    Ok(question.source_digest)
}
