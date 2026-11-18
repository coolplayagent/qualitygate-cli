//! Timed adapter entry point for V4 assertions over captured collections.

pub use super::assertion_v4::{Collection, Location, Observation, Outcome, Record};
use crate::{domain::operator::MAX_SECONDS, rule_catalog::operator_v4::Expr};
use anyhow::Result;
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

pub fn evaluate(expression: &Expr, collections: &BTreeMap<String, Collection>) -> Result<Outcome> {
    evaluate_since(expression, collections, Instant::now())
}

fn evaluate_since(
    expression: &Expr,
    collections: &BTreeMap<String, Collection>,
    started: Instant,
) -> Result<Outcome> {
    super::assertion_v4::evaluate_with_budget(expression, collections, &|| {
        started.elapsed() >= Duration::from_secs(MAX_SECONDS)
    })
}

#[cfg(test)]
#[path = "operators_v4_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "operators_v4_regression_tests.rs"]
mod regression_tests;
