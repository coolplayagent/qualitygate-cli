//! Bound retained diagnostic JSON before appending records or repeating replay argv.
use crate::domain::{
    CheckResult, Diagnostic, ExecutionStatus, MAX_DIAGNOSTIC_BYTES, MAX_DIAGNOSTICS,
};
use anyhow::{Result, ensure};
use std::io::{self, Write};
use std::{
    ops::{Deref, DerefMut},
    sync::Mutex,
    time::Instant,
};

pub(crate) struct Budget {
    count: usize,
    bytes: usize,
    max_count: usize,
    max_bytes: usize,
    exhausted: bool,
}
impl Default for Budget {
    fn default() -> Self {
        Self {
            count: 0,
            bytes: 2,
            max_count: MAX_DIAGNOSTICS,
            max_bytes: MAX_DIAGNOSTIC_BYTES,
            exhausted: false,
        }
    }
}
impl Budget {
    fn charge(&mut self, diagnostic: &Diagnostic) -> Result<()> {
        let result = self.charge_inner(diagnostic);
        self.exhausted |= result.is_err();
        result
    }

    fn charge_inner(&mut self, diagnostic: &Diagnostic) -> Result<()> {
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
                "complete":false,"retained":result.diagnostics.len(),"phase":phase,"encoding":"compact_json_utf8",
                "max_records":self.max_count,"max_serialized_bytes":self.max_bytes,
                "omitted_count_known":false
            }),
        );
    }
}

/// All built-ins share one output budget and analysis deadline. Workers reserve
/// before retaining a finding, so parallel batches cannot each consume the cap.
pub(crate) struct BuiltinResult {
    result: CheckResult,
    budget: Mutex<Budget>,
    deadline: Instant,
}

impl BuiltinResult {
    pub(crate) fn new(result: CheckResult) -> Self {
        Self {
            result,
            budget: Mutex::new(Budget::default()),
            deadline: crate::adapters::parallel::deadline(),
        }
    }

    pub(crate) fn deadline(&self) -> Instant {
        self.deadline
    }

    pub(crate) fn check_deadline(&self) -> Result<()> {
        crate::adapters::parallel::check_deadline(self.deadline)
    }

    fn reserve(&self, diagnostic: &Diagnostic) -> Result<()> {
        self.check_deadline()?;
        self.budget
            .lock()
            .map_err(|_| anyhow::anyhow!("Diagnostic budget state unavailable"))?
            .charge(diagnostic)
            .map_err(|error| {
                anyhow::anyhow!("Built-in diagnostics exceed 10000 records or 8 MiB: {error}")
            })
    }

    pub(crate) fn push(&mut self, diagnostic: Diagnostic) -> Result<()> {
        self.reserve(&diagnostic)?;
        self.result.diagnostics.push(diagnostic);
        Ok(())
    }

    pub(crate) fn push_buffered(
        &self,
        output: &mut Vec<Diagnostic>,
        diagnostic: Diagnostic,
    ) -> Result<()> {
        self.reserve(&diagnostic)?;
        output.push(diagnostic);
        Ok(())
    }

    pub(crate) fn finish(mut self) -> CheckResult {
        if let Ok(budget) = self.budget.lock()
            && budget.exhausted
        {
            budget.incomplete(&mut self.result, "rule_evaluation");
        }
        self.result
    }
}

impl Deref for BuiltinResult {
    type Target = CheckResult;
    fn deref(&self) -> &Self::Target {
        &self.result
    }
}
impl DerefMut for BuiltinResult {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.result
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
