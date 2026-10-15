//! Bounded expression evaluation over already acquired, complete entity facts.
#[cfg(test)]
#[path = "operators_tests.rs"]
mod tests;
use crate::domain::operator::*;
use anyhow::{Result, bail, ensure};
use serde::Serialize;
use std::time::{Duration, Instant};

pub struct Entity<'a> {
    pub name: &'a str,
    pub text: &'a str,
    pub path: Option<&'a str>,
    pub range: Option<&'a crate::domain::Range>,
}
pub struct Facts<'a> {
    pub entities: &'a [Entity<'a>],
    pub complete: bool,
}

#[derive(Debug, Serialize)]
pub struct Outcome {
    pub passed: bool,
    pub operations: usize,
    pub scanned_bytes: usize,
    pub entities: usize,
    pub false_observations: Vec<Observation>,
    pub omitted_observations: usize,
}

/// False subexpressions are observations, not necessarily whole-rule violations.
#[derive(Debug, Serialize)]
pub struct Observation {
    pub expression_path: String,
    pub entity_index: Option<usize>,
    pub name: Option<String>,
    pub name_truncated: bool,
    pub file: Option<String>,
    pub file_truncated: bool,
    pub range: Option<crate::domain::Range>,
}

enum Value<'a> {
    Bool(bool),
    Number(i64),
    String(&'a str),
    Collection(Vec<usize>),
}

impl<'a> Value<'a> {
    fn boolean(self) -> Result<bool> {
        match self {
            Self::Bool(v) => Ok(v),
            _ => bail!("Expected boolean expression value"),
        }
    }
    fn collection(self) -> Result<Vec<usize>> {
        match self {
            Self::Collection(v) => Ok(v),
            _ => bail!("Expected entity collection"),
        }
    }
    fn string(self) -> Result<&'a str> {
        match self {
            Self::String(v) => Ok(v),
            _ => bail!("Expected string expression value"),
        }
    }
    fn scalar(value: &Scalar) -> Value<'_> {
        match value {
            Scalar::Bool(v) => Value::Bool(*v),
            Scalar::Number(v) => Value::Number(*v),
            Scalar::String(v) => Value::String(v),
        }
    }
    fn bytes(&self) -> usize {
        if let Self::String(s) = self {
            s.len()
        } else {
            0
        }
    }
}

struct Engine<'a> {
    entities: &'a [Entity<'a>],
    checked: Validated,
    started: Instant,
    operations: usize,
    scanned_bytes: usize,
    observations: Vec<Observation>,
    omitted_observations: usize,
}

pub fn evaluate(expression: &Expr, facts: Facts<'_>) -> Result<Outcome> {
    evaluate_since(expression, facts, Instant::now())
}

fn evaluate_since(expression: &Expr, facts: Facts<'_>, started: Instant) -> Result<Outcome> {
    let checked = expression.validate().map_err(anyhow::Error::msg)?;
    // All required inputs are checked before boolean evaluation, even for a constant branch.
    ensure!(facts.complete, "Expression entity facts are incomplete");
    ensure!(
        facts.entities.len() <= MAX_ENTITIES,
        "Expression exceeds 50000 entities"
    );
    let mut total = 0usize;
    for entity in facts.entities {
        ensure!(
            !checked.requires_path || entity.path.is_some(),
            "Required entity path fact is unavailable"
        );
        total = total
            .saturating_add(entity.name.len())
            .saturating_add(entity.text.len())
            .saturating_add(entity.path.map_or(0, str::len));
        ensure!(total <= MAX_FACT_BYTES, "Expression facts exceed 32 MiB");
    }
    let mut engine = Engine {
        entities: facts.entities,
        checked,
        started,
        operations: 0,
        scanned_bytes: 0,
        observations: Vec::new(),
        omitted_observations: 0,
    };
    let passed = engine.eval(expression, None, "/expression")?.boolean()?;
    engine.deadline()?;
    Ok(Outcome {
        passed,
        operations: engine.operations,
        scanned_bytes: engine.scanned_bytes,
        entities: facts.entities.len(),
        false_observations: engine.observations,
        omitted_observations: engine.omitted_observations,
    })
}

impl<'a> Engine<'a> {
    fn observe(&mut self, location: &str, current: Option<usize>) {
        if self.observations.len() >= 64 {
            self.omitted_observations += 1;
            return;
        }
        fn preview(value: &str, limit: usize) -> (String, bool) {
            let mut chars = value.chars();
            let prefix = chars.by_ref().take(limit).collect();
            (prefix, chars.next().is_some())
        }
        let entity = current.and_then(|i| self.entities.get(i));
        let name = entity.map(|e| preview(e.name, 256));
        let file = entity.and_then(|e| e.path).map(|p| preview(p, 1024));
        self.observations.push(Observation {
            expression_path: location.into(),
            entity_index: current,
            name_truncated: name.as_ref().is_some_and(|(_, t)| *t),
            name: name.map(|(s, _)| s),
            file_truncated: file.as_ref().is_some_and(|(_, t)| *t),
            file: file.map(|(s, _)| s),
            range: entity.and_then(|e| e.range).cloned(),
        });
    }
    fn deadline(&self) -> Result<()> {
        ensure!(
            self.started.elapsed() < Duration::from_secs(MAX_SECONDS),
            "Expression exceeded its 30-second deadline"
        );
        Ok(())
    }
    fn step(&mut self, count: usize) -> Result<()> {
        self.deadline()?;
        self.operations = self.operations.saturating_add(count);
        ensure!(
            self.operations <= MAX_STEPS,
            "Expression exceeded 100000 operations"
        );
        Ok(())
    }
    fn scan(&mut self, count: usize) -> Result<()> {
        self.scanned_bytes = self.scanned_bytes.saturating_add(count);
        ensure!(
            self.scanned_bytes <= MAX_SCAN_BYTES,
            "Expression exceeded 32 MiB scanned bytes"
        );
        self.deadline()
    }
    fn compare(&mut self, left: &Value<'_>, relation: Relation, right: &Value<'_>) -> Result<bool> {
        self.scan(left.bytes().saturating_add(right.bytes()))?;
        let order = match (left, right) {
            (Value::Number(a), Value::Number(b)) => a.cmp(b),
            (Value::String(a), Value::String(b))
                if matches!(relation, Relation::Eq | Relation::Ne) =>
            {
                a.cmp(b)
            }
            (Value::Bool(a), Value::Bool(b)) if matches!(relation, Relation::Eq | Relation::Ne) => {
                a.cmp(b)
            }
            _ => bail!("Inconsistent typed comparison"),
        };
        Ok(match relation {
            Relation::Eq => order.is_eq(),
            Relation::Ne => !order.is_eq(),
            Relation::Lt => order.is_lt(),
            Relation::Le => !order.is_gt(),
            Relation::Gt => order.is_gt(),
            Relation::Ge => !order.is_lt(),
        })
    }
    fn eval<'e>(
        &mut self,
        expression: &'e Expr,
        current: Option<usize>,
        location: &str,
    ) -> Result<Value<'e>>
    where
        'a: 'e,
    {
        self.step(1)?;
        let value: Result<Value<'e>> = match expression {
            Expr::Bool { value } => Ok(Value::Bool(*value)),
            Expr::Number { value } => Ok(Value::Number(*value)),
            Expr::String { value } => Ok(Value::String(value)),
            Expr::Field { name } => {
                let entity = current
                    .and_then(|i| self.entities.get(i))
                    .ok_or_else(|| anyhow::anyhow!("Missing bound entity"))?;
                Ok(Value::String(match name {
                    Field::Name => entity.name,
                    Field::Text => entity.text,
                    Field::Path => entity
                        .path
                        .ok_or_else(|| anyhow::anyhow!("Missing entity path"))?,
                }))
            }
            Expr::Entities => {
                self.step(self.entities.len())?;
                Ok(Value::Collection((0..self.entities.len()).collect()))
            }
            Expr::And { args } | Expr::Or { args } => {
                let conjunction = matches!(expression, Expr::And { .. });
                let mut result = conjunction;
                for (index, arg) in args.iter().enumerate() {
                    let value = self
                        .eval(arg, current, &format!("{location}/args/{index}"))?
                        .boolean()?;
                    result = if conjunction {
                        result & value
                    } else {
                        result | value
                    };
                }
                Ok(Value::Bool(result))
            }
            Expr::Not { arg } => Ok(Value::Bool(
                !self
                    .eval(arg, current, &format!("{location}/arg"))?
                    .boolean()?,
            )),
            Expr::All { input, predicate }
            | Expr::Any { input, predicate }
            | Expr::Filter { input, predicate } => {
                let collection = self
                    .eval(input, current, &format!("{location}/input"))?
                    .collection()?;
                let all = matches!(expression, Expr::All { .. });
                let filter = matches!(expression, Expr::Filter { .. });
                let mut result = all;
                let mut selected = Vec::new();
                for index in collection {
                    self.step(1)?;
                    let value = self
                        .eval(predicate, Some(index), &format!("{location}/predicate"))?
                        .boolean()?;
                    result = if all { result & value } else { result | value };
                    if filter && value {
                        selected.push(index);
                    }
                }
                Ok(if filter {
                    Value::Collection(selected)
                } else {
                    Value::Bool(result)
                })
            }
            Expr::Count { input } => Ok(Value::Number(
                self.eval(input, current, &format!("{location}/input"))?
                    .collection()?
                    .len() as i64,
            )),
            Expr::Compare {
                left,
                relation,
                right,
            } => {
                let left = self.eval(left, current, &format!("{location}/left"))?;
                let right = self.eval(right, current, &format!("{location}/right"))?;
                Ok(Value::Bool(self.compare(&left, *relation, &right)?))
            }
            Expr::In { value, set } => {
                let value = self.eval(value, current, &format!("{location}/value"))?;
                let mut found = false;
                for item in set {
                    self.step(1)?;
                    found |= self.compare(&value, Relation::Eq, &Value::scalar(item))?;
                }
                Ok(Value::Bool(found))
            }
            Expr::Matches { value, pattern } => {
                let value = self
                    .eval(value, current, &format!("{location}/value"))?
                    .string()?;
                self.scan(value.len())?;
                let regex = self
                    .checked
                    .patterns
                    .get(pattern)
                    .ok_or_else(|| anyhow::anyhow!("Missing validated regex"))?;
                Ok(Value::Bool(regex.is_match(value)))
            }
        };
        let value = value?;
        if matches!(value, Value::Bool(false)) {
            self.observe(location, current);
        }
        Ok(value)
    }
}
