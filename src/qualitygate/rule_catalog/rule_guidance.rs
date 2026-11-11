//! Literal, builtin-owned diagnostic presentation; it cannot change a verdict.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

pub const MAX_TEXT_BYTES: usize = 4096;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Diagnostic {
    pub message: String,
    pub fix: String,
}

impl Diagnostic {
    pub fn validate(&self) -> Result<()> {
        for (name, text) in [("message", &self.message), ("fix", &self.fix)] {
            ensure!(
                !text.trim().is_empty() && text.len() <= MAX_TEXT_BYTES,
                "Builtin diagnostic {name} must contain 1..{MAX_TEXT_BYTES} UTF-8 bytes and nonblank text"
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_guidance_is_closed_and_bounded_by_utf8_bytes() {
        let good = Diagnostic {
            message: "Review the evidence".into(),
            fix: "Inspect the call".into(),
        };
        good.validate().unwrap();
        for invalid in [
            String::new(),
            " \n\t".into(),
            "界".repeat(MAX_TEXT_BYTES / 3 + 1),
        ] {
            assert!(
                Diagnostic {
                    message: invalid.clone(),
                    ..good.clone()
                }
                .validate()
                .is_err()
            );
            assert!(
                Diagnostic {
                    fix: invalid,
                    ..good.clone()
                }
                .validate()
                .is_err()
            );
        }
        Diagnostic {
            message: "x".repeat(MAX_TEXT_BYTES),
            ..good
        }
        .validate()
        .unwrap();
        for invalid in [
            r#"{"message":"review","fix":"inspect","template":true}"#,
            r#"{"message":"review"}"#,
        ] {
            assert!(serde_json::from_str::<Diagnostic>(invalid).is_err());
        }
    }
}
