//! Bound serialized report evidence without allocating a JSON copy.

use anyhow::Result;
use serde::Serialize;
use std::io::{self, Write};

pub(super) const MAX_BYTES: usize = 16 * 1024 * 1024;

pub(super) struct Budget {
    remaining: usize,
    subject: &'static str,
}

impl Budget {
    pub(super) fn new(subject: &'static str) -> Self {
        Self {
            remaining: MAX_BYTES,
            subject,
        }
    }

    pub(super) fn charge(&mut self, value: &impl Serialize) -> Result<()> {
        serde_json::to_writer(self, value)?;
        Ok(())
    }

    pub(super) fn push<T: Serialize>(&mut self, retained: &mut Vec<T>, value: T) -> Result<()> {
        self.charge(&value)?;
        retained.push(value);
        Ok(())
    }
}

impl Write for Budget {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.remaining = self
            .remaining
            .checked_sub(bytes.len())
            .ok_or_else(|| io::Error::other(format!("{} exceed 16 MiB", self.subject)))?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialized_budget_counts_json_escaping_and_rejects_before_copying_output() {
        let mut budget = Budget {
            remaining: 8,
            subject: "results",
        };
        budget.charge(&"\u{0001}").unwrap();
        assert_eq!(budget.remaining, 0);
        assert!(
            budget
                .charge(&0)
                .unwrap_err()
                .to_string()
                .contains("16 MiB")
        );

        let mut budget = Budget::new("locations");
        budget.charge(&"x".repeat(MAX_BYTES - 2)).unwrap();
        assert_eq!(budget.remaining, 0);
        assert!(budget.charge(&0).is_err());
    }

    #[test]
    fn rejected_values_are_not_appended_to_retained_evidence() {
        let mut budget = Budget {
            remaining: 8,
            subject: "Normalized diagnostics",
        };
        let mut retained = Vec::new();
        budget.push(&mut retained, "1234").unwrap();
        assert!(budget.push(&mut retained, "abc").is_err());
        assert_eq!(retained, ["1234"]);
    }
}
