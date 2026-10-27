//! Bound retained diagnostic JSON before appending records or repeating replay argv.
use crate::domain::{
    CheckResult, Diagnostic, ExecutionStatus, MAX_DIAGNOSTIC_BYTES, MAX_DIAGNOSTICS,
};
use anyhow::{Result, ensure};
use std::io::{self, Write};

pub(crate) struct Budget {
    count: usize,
    bytes: usize,
    max_count: usize,
    max_bytes: usize,
}
impl Default for Budget {
    fn default() -> Self {
        Self {
            count: 0,
            bytes: 2,
            max_count: MAX_DIAGNOSTICS,
            max_bytes: MAX_DIAGNOSTIC_BYTES,
        }
    }
}
impl Budget {
    fn charge(&mut self, diagnostic: &Diagnostic) -> Result<()> {
        ensure!(
            self.count < self.max_count,
            "Diagnostic output exceeds {} records",
            self.max_count
        );
        let separator = usize::from(self.count != 0);
        let mut meter = Meter {
            bytes: self.bytes.saturating_add(separator),
            limit: self.max_bytes,
        };
        serde_json::to_writer(&mut meter, diagnostic)?;
        self.bytes = meter.bytes;
        self.count += 1;
        Ok(())
    }

    pub(crate) fn push(&mut self, result: &mut CheckResult, diagnostic: Diagnostic) -> Result<()> {
        if let Err(error) = self.charge(&diagnostic) {
            self.incomplete(result, "rule_evaluation");
            return Err(error);
        }
        result.diagnostics.push(diagnostic);
        Ok(())
    }

    fn incomplete(&self, result: &mut CheckResult, phase: &str) {
        result.metadata.insert(
            "diagnostic_output".into(),
            serde_json::json!({
                "complete":false,"retained":self.count,"phase":phase,"encoding":"compact_json_utf8",
                "max_records":self.max_count,"max_serialized_bytes":self.max_bytes,
                "omitted_count_known":false
            }),
        );
    }
}

struct Meter {
    bytes: usize,
    limit: usize,
}
impl Write for Meter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.limit.saturating_sub(self.bytes) {
            return Err(io::Error::other(format!(
                "Diagnostic output exceeds {} serialized JSON bytes",
                self.limit
            )));
        }
        self.bytes += bytes.len();
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Replay arguments are also charged; an incomplete result keeps its original failure reason.
pub fn attach_recheck(result: &mut CheckResult, argv: &[String]) {
    attach(result, argv, Budget::default());
}
fn attach(result: &mut CheckResult, argv: &[String], mut budget: Budget) {
    for index in 0..result.diagnostics.len() {
        result.diagnostics[index].recheck.argv = argv.to_vec();
        if let Err(error) = budget.charge(&result.diagnostics[index]) {
            result.diagnostics.truncate(index);
            budget.incomplete(result, "recheck_binding");
            let reason = match &result.execution.reason {
                Some(previous) => format!("{previous}; {error:#}"),
                None => format!("{error:#}"),
            };
            let status = match result.execution.status {
                ExecutionStatus::Completed | ExecutionStatus::NotRun => ExecutionStatus::Blocked,
                status => status,
            };
            result.block(status, reason);
            return;
        }
    }
}

#[cfg(test)]
#[path = "diagnostic_budget_tests.rs"]
mod tests;
