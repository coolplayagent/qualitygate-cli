//! Bounded lexical evaluation over complete, snapshot-owned named collections.
use super::operators::Entity;
use crate::domain::{
    Range,
    operator::*,
    operator_v3::{Expr, Validated},
};
use anyhow::{Result, bail, ensure};
use serde::Serialize;
use std::{
    cmp::Ordering,
    collections::BTreeMap,
    sync::Arc,
    time::{Duration, Instant},
};

pub struct Collection<'a> {
    pub entities: Vec<Entity<'a>>,
    pub complete: bool,
}
#[derive(Debug, Serialize)]
pub struct Outcome {
    pub passed: bool,
    pub operations: usize,
    pub scanned_bytes: usize,
    pub materialized_bytes: usize,
    pub entities: usize,
    pub sources: BTreeMap<String, usize>,
    pub false_observations: Vec<Observation>,
    pub omitted_observations: usize,
}
#[derive(Debug, Serialize)]
pub struct Location {
    pub source: String,
    pub entity_index: usize,
    pub file: Option<String>,
    pub file_truncated: bool,
    pub range: Option<Range>,
}
#[derive(Debug, Serialize)]
pub struct Observation {
    pub expression_path: String,
    pub current: Option<Location>,
    pub bindings: BTreeMap<String, Location>,
}
#[derive(Clone)]
enum Atom {
    Bool(bool),
    Number(i64),
    String(Arc<str>),
    Entity(usize),
}
#[derive(Clone)]
struct Item {
    atom: Atom,
    origin: Option<usize>,
}
enum Value {
    Item(Item),
    Collection(Vec<Item>),
}
enum ScalarIndex {
    Small(Vec<usize>),
    Ordered(Vec<usize>),
}
impl Value {
    fn item(self) -> Result<Item> {
        match self {
            Self::Item(v) => Ok(v),
            _ => bail!("Expected scalar or entity"),
        }
    }
    fn collection(self) -> Result<Vec<Item>> {
        match self {
            Self::Collection(v) => Ok(v),
            _ => bail!("Expected collection"),
        }
    }
    fn boolean(self) -> Result<bool> {
        match self.item()?.atom {
            Atom::Bool(v) => Ok(v),
            _ => bail!("Expected boolean"),
        }
    }
    fn string(self) -> Result<Arc<str>> {
        match self.item()?.atom {
            Atom::String(v) => Ok(v),
            _ => bail!("Expected string"),
        }
    }
    fn scalar(atom: Atom, current: Option<&Item>) -> Self {
        Self::Item(Item {
            atom,
            origin: current.and_then(|v| v.origin),
        })
    }
}
struct Fact<'a> {
    entity: &'a Entity<'a>,
    source: &'a str,
    index: usize,
}
struct Engine<'a> {
    facts: Vec<Fact<'a>>,
    sources: BTreeMap<String, Vec<usize>>,
    checked: Validated,
    variables: BTreeMap<String, Item>,
    started: Instant,
    operations: usize,
    scanned_bytes: usize,
    materialized_bytes: usize,
    observations: Vec<Observation>,
    omitted: usize,
}

pub fn evaluate(
    expression: &Expr,
    collections: &BTreeMap<String, Collection<'_>>,
) -> Result<Outcome> {
    evaluate_since(expression, collections, Instant::now())
}
fn evaluate_since(
    expression: &Expr,
    collections: &BTreeMap<String, Collection<'_>>,
    started: Instant,
) -> Result<Outcome> {
    ensure!(
        collections.contains_key("entities"),
        "Missing primary entity collection"
    );
    let checked = expression
        .validate(
            &collections
                .keys()
                .filter(|s| *s != "entities")
                .cloned()
                .collect(),
        )
        .map_err(anyhow::Error::msg)?;
    let mut facts = Vec::new();
    let mut sources = BTreeMap::new();
    let mut bytes = 0usize;
    for (name, collection) in collections {
        ensure!(collection.complete, "Source {name} is incomplete");
        let mut indices = Vec::new();
        for (index, entity) in collection.entities.iter().enumerate() {
            ensure!(
                started.elapsed() < Duration::from_secs(MAX_SECONDS),
                "Expression exceeded its 30-second deadline"
            );
            ensure!(
                !checked.path_sources.contains(name) || entity.path.is_some(),
                "Required path fact unavailable in source {name}"
            );
            bytes = bytes
                .saturating_add(entity.name.len())
                .saturating_add(entity.text.len())
                .saturating_add(entity.path.map_or(0, str::len));
            ensure!(
                bytes <= MAX_FACT_BYTES && facts.len() < MAX_ENTITIES,
                "Expression sources exceed 50000 entities or 32 MiB facts"
            );
            indices.push(facts.len());
            facts.push(Fact {
                entity,
                source: name,
                index,
            });
        }
        sources.insert(name.clone(), indices);
    }
    let counts = sources
        .iter()
        .map(|(name, values)| (name.clone(), values.len()))
        .collect();
    let mut engine = Engine {
        facts,
        sources,
        checked,
        variables: BTreeMap::new(),
        started,
        operations: 0,
        scanned_bytes: 0,
        materialized_bytes: 0,
        observations: Vec::new(),
        omitted: 0,
    };
    let passed = engine.eval(expression, None, "/expression")?.boolean()?;
    engine.deadline()?;
    Ok(Outcome {
        passed,
        operations: engine.operations,
        scanned_bytes: engine.scanned_bytes,
        materialized_bytes: engine.materialized_bytes,
        entities: engine.facts.len(),
        sources: counts,
        false_observations: engine.observations,
        omitted_observations: engine.omitted,
    })
}
impl Engine<'_> {
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
    fn scan(&mut self, bytes: usize) -> Result<()> {
        self.deadline()?;
        self.scanned_bytes = self.scanned_bytes.saturating_add(bytes);
        ensure!(
            self.scanned_bytes <= MAX_SCAN_BYTES,
            "Expression exceeded 32 MiB scanned bytes"
        );
        Ok(())
    }
    fn allocate(&mut self, bytes: usize) -> Result<()> {
        self.deadline()?;
        self.materialized_bytes = self.materialized_bytes.saturating_add(bytes);
        ensure!(
            self.materialized_bytes <= MAX_FACT_BYTES,
            "Expression exceeded 32 MiB materialized bytes"
        );
        Ok(())
    }
    fn string(&mut self, text: &str, current: Option<&Item>) -> Result<Value> {
        self.allocate(text.len())?;
        Ok(Value::scalar(Atom::String(Arc::from(text)), current))
    }
    fn push(&mut self, output: &mut Vec<Item>, item: Item) -> Result<()> {
        ensure!(
            output.len() < MAX_ENTITIES,
            "Intermediate collection exceeds 50000 items"
        );
        self.allocate(
            std::mem::size_of::<Item>()
                + match &item.atom {
                    Atom::String(s) => s.len(),
                    _ => 0,
                },
        )?;
        output.push(item);
        Ok(())
    }
    fn source(&mut self, name: &str) -> Result<Value> {
        let len = self
            .sources
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("Unavailable source: {name}"))?
            .len();
        self.step(len)?;
        self.allocate(len.saturating_mul(std::mem::size_of::<Item>()))?;
        Ok(Value::Collection(
            self.sources[name]
                .iter()
                .map(|i| Item {
                    atom: Atom::Entity(*i),
                    origin: Some(*i),
                })
                .collect(),
        ))
    }
    fn location(&self, index: usize) -> Location {
        let fact = &self.facts[index];
        let file = fact.entity.path.map(|p| p.chars().take(1024).collect());
        Location {
            source: fact.source.into(),
            entity_index: fact.index,
            file,
            file_truncated: fact.entity.path.is_some_and(|p| p.chars().count() > 1024),
            range: fact.entity.range.cloned(),
        }
    }
    fn observe(&mut self, pointer: &str, current: Option<&Item>) {
        if self.observations.len() >= 64 {
            self.omitted += 1;
            return;
        }
        self.observations.push(Observation {
            expression_path: pointer.into(),
            current: current.and_then(|v| v.origin).map(|i| self.location(i)),
            bindings: self
                .variables
                .iter()
                .filter_map(|(name, item)| item.origin.map(|i| (name.clone(), self.location(i))))
                .collect(),
        });
    }
    fn order(&mut self, a: &Item, b: &Item) -> Result<Ordering> {
        self.step(1)?;
        Ok(match (&a.atom, &b.atom) {
            (Atom::Bool(a), Atom::Bool(b)) => a.cmp(b),
            (Atom::Number(a), Atom::Number(b)) => a.cmp(b),
            (Atom::String(a), Atom::String(b)) => {
                self.scan(a.len().saturating_add(b.len()))?;
                a.cmp(b)
            }
            _ => bail!("Inconsistent scalar comparison"),
        })
    }
    fn compare(&mut self, a: &Item, relation: Relation, b: &Item) -> Result<bool> {
        let order = self.order(a, b)?;
        Ok(match relation {
            Relation::Eq => order.is_eq(),
            Relation::Ne => !order.is_eq(),
            Relation::Lt => order.is_lt(),
            Relation::Le => !order.is_gt(),
            Relation::Gt => order.is_gt(),
            Relation::Ge => !order.is_lt(),
        })
    }
    fn contains(&mut self, items: &[Item], value: &Item) -> Result<bool> {
        let mut found = false;
        for item in items {
            found |= self.compare(item, Relation::Eq, value)?;
        }
        Ok(found)
    }

    fn contains_indices(
        &mut self,
        items: &[Item],
        indices: &[usize],
        value: &Item,
    ) -> Result<bool> {
        self.deadline()?;
        let mut found = false;
        for &index in indices {
            found |= self.compare(&items[index], Relation::Eq, value)?;
        }
        Ok(found)
    }

    fn scalar_index(&mut self, items: &[Item]) -> Result<ScalarIndex> {
        // Few distinct values need only linear work, even with many duplicates.
        // Bound this probe before choosing the general ordered index.
        let mut indices = Vec::new();
        for index in 0..items.len() {
            if !self.contains_indices(items, &indices, &items[index])? {
                if indices.len() == 16 {
                    return Ok(ScalarIndex::Ordered(self.ordered_indices(items)?));
                }
                self.allocate(std::mem::size_of::<usize>())?;
                indices.push(index);
            }
        }
        Ok(ScalarIndex::Small(indices))
    }

    /// Stable merge ordering permits each comparison and index move to fail at
    /// the shared budget boundary. Sorting indices leaves values and origins intact.
    fn ordered_indices(&mut self, items: &[Item]) -> Result<Vec<usize>> {
        let len = items.len();
        self.allocate(len.saturating_mul(2 * std::mem::size_of::<usize>()))?;
        self.step(len)?;
        let mut indices: Vec<_> = (0..len).collect();
        let mut scratch = vec![0; len];
        let mut width = 1;
        while width < len {
            for start in (0..len).step_by(2 * width) {
                let middle = (start + width).min(len);
                let end = (middle + width).min(len);
                let (mut left, mut right) = (start, middle);
                for slot in &mut scratch[start..end] {
                    self.step(1)?;
                    let take_left = right == end
                        || left < middle
                            && self.order(&items[indices[left]], &items[indices[right]])?
                                != Ordering::Greater;
                    *slot = if take_left {
                        let index = indices[left];
                        left += 1;
                        index
                    } else {
                        let index = indices[right];
                        right += 1;
                        index
                    };
                }
            }
            std::mem::swap(&mut indices, &mut scratch);
            width *= 2;
        }
        Ok(indices)
    }

    fn distinct(&mut self, items: Vec<Item>) -> Result<Vec<Item>> {
        let indices = match self.scalar_index(&items)? {
            ScalarIndex::Ordered(indices) => indices,
            ScalarIndex::Small(indices) => {
                let mut output = Vec::new();
                for index in indices {
                    self.push(&mut output, items[index].clone())?;
                }
                return Ok(output);
            }
        };
        self.allocate(items.len())?;
        let mut keep = vec![false; items.len()];
        let mut previous: Option<usize> = None;
        for index in indices {
            self.step(1)?;
            let unique = match previous {
                Some(previous) => self.order(&items[previous], &items[index])? != Ordering::Equal,
                None => true,
            };
            if unique {
                keep[index] = true;
                previous = Some(index);
            }
        }
        let mut output = Vec::new();
        for (index, item) in items.into_iter().enumerate() {
            self.step(1)?;
            if keep[index] {
                self.push(&mut output, item)?;
            }
        }
        Ok(output)
    }

    fn indexed_contains(
        &mut self,
        items: &[Item],
        index: &ScalarIndex,
        value: &Item,
    ) -> Result<bool> {
        let indices = match index {
            ScalarIndex::Small(indices) => return self.contains_indices(items, indices, value),
            ScalarIndex::Ordered(indices) => indices,
        };
        let (mut start, mut end) = (0, indices.len());
        while start < end {
            let middle = start + (end - start) / 2;
            match self.order(&items[indices[middle]], value)? {
                Ordering::Equal => return Ok(true),
                Ordering::Less => start = middle + 1,
                Ordering::Greater => end = middle,
            }
        }
        Ok(false)
    }
    fn eval(&mut self, expression: &Expr, current: Option<&Item>, pointer: &str) -> Result<Value> {
        self.step(1)?;
        use Expr::*;
        let value = match expression {
            Bool { value } => Value::scalar(Atom::Bool(*value), current),
            Number { value } => Value::scalar(Atom::Number(*value), current),
            String { value } => self.string(value, current)?,
            Entities => self.source("entities")?,
            Source { name } => self.source(name)?,
            Var { name } => Value::Item(
                self.variables
                    .get(name)
                    .ok_or_else(|| anyhow::anyhow!("Unbound variable: {name}"))?
                    .clone(),
            ),
            Field { name, of } => {
                let item = if let Some(of) = of {
                    self.eval(of, current, &format!("{pointer}/of"))?.item()?
                } else {
                    current
                        .cloned()
                        .ok_or_else(|| anyhow::anyhow!("Missing entity binding"))?
                };
                let Atom::Entity(index) = item.atom else {
                    bail!("Field target must be an entity");
                };
                let entity = self.facts[index].entity;
                let text = match name {
                    crate::domain::operator::Field::Name => entity.name,
                    crate::domain::operator::Field::Text => entity.text,
                    crate::domain::operator::Field::Path => entity
                        .path
                        .ok_or_else(|| anyhow::anyhow!("Missing path fact"))?,
                };
                self.string(text, Some(&item))?
            }
            And { args } | Or { args } => {
                let all = matches!(expression, And { .. });
                let mut result = all;
                for (index, arg) in args.iter().enumerate() {
                    let v = self
                        .eval(arg, current, &format!("{pointer}/args/{index}"))?
                        .boolean()?;
                    if all {
                        result &= v;
                    } else {
                        result |= v;
                    }
                }
                Value::scalar(Atom::Bool(result), current)
            }
            Not { arg } => {
                let value = self
                    .eval(arg, current, &format!("{pointer}/arg"))?
                    .boolean()?;
                Value::scalar(Atom::Bool(!value), current)
            }
            All {
                input,
                binding,
                predicate,
            }
            | Any {
                input,
                binding,
                predicate,
            }
            | Filter {
                input,
                binding,
                predicate,
            } => {
                let items = self
                    .eval(input, current, &format!("{pointer}/input"))?
                    .collection()?;
                let mut result = matches!(expression, All { .. });
                let mut output = Vec::new();
                for item in items {
                    self.step(1)?;
                    if let Some(name) = binding {
                        self.variables.insert(name.clone(), item.clone());
                    }
                    let outcome =
                        self.eval(predicate, Some(&item), &format!("{pointer}/predicate"));
                    if let Some(name) = binding {
                        self.variables.remove(name);
                    }
                    let passed = outcome?.boolean()?;
                    match expression {
                        All { .. } => result &= passed,
                        Any { .. } => result |= passed,
                        _ => {
                            if passed {
                                self.push(&mut output, item)?;
                            }
                        }
                    }
                }
                if matches!(expression, Filter { .. }) {
                    Value::Collection(output)
                } else {
                    Value::scalar(Atom::Bool(result), current)
                }
            }
            Map {
                input,
                binding,
                body,
            } => {
                let items = self
                    .eval(input, current, &format!("{pointer}/input"))?
                    .collection()?;
                let mut output = Vec::new();
                for item in items {
                    self.step(1)?;
                    self.variables.insert(binding.clone(), item.clone());
                    let outcome = self.eval(body, Some(&item), &format!("{pointer}/body"));
                    self.variables.remove(binding);
                    self.push(&mut output, outcome?.item()?)?;
                }
                Value::Collection(output)
            }
            Count { input } => {
                let items = self
                    .eval(input, current, &format!("{pointer}/input"))?
                    .collection()?;
                Value::scalar(Atom::Number(items.len() as i64), current)
            }
            Distinct { input } => {
                let items = self
                    .eval(input, current, &format!("{pointer}/input"))?
                    .collection()?;
                Value::Collection(self.distinct(items)?)
            }
            Contains { input, value } => {
                let items = self
                    .eval(input, current, &format!("{pointer}/input"))?
                    .collection()?;
                let value = self
                    .eval(value, current, &format!("{pointer}/value"))?
                    .item()?;
                Value::scalar(Atom::Bool(self.contains(&items, &value)?), current)
            }
            Subset { left, right } => {
                let left = self
                    .eval(left, current, &format!("{pointer}/left"))?
                    .collection()?;
                let right = self
                    .eval(right, current, &format!("{pointer}/right"))?
                    .collection()?;
                let mut result = true;
                // A few full scans cost less than building the ordered index.
                // Estimate its comparison and move passes before indexing.
                let scan_limit = 2 * right.len().max(1).ilog2() as usize + 1;
                if left.len() <= scan_limit {
                    for item in &left {
                        result &= self.contains(&right, item)?;
                    }
                } else {
                    let index = self.scalar_index(&right)?;
                    for item in &left {
                        result &= self.indexed_contains(&right, &index, item)?;
                    }
                }
                Value::scalar(Atom::Bool(result), current)
            }
            Compare {
                left,
                relation,
                right,
            } => {
                let left = self
                    .eval(left, current, &format!("{pointer}/left"))?
                    .item()?;
                let right = self
                    .eval(right, current, &format!("{pointer}/right"))?
                    .item()?;
                Value::scalar(Atom::Bool(self.compare(&left, *relation, &right)?), current)
            }
            In { value, set } => {
                let value = self
                    .eval(value, current, &format!("{pointer}/value"))?
                    .item()?;
                let mut found = false;
                for scalar in set {
                    let item = match scalar {
                        Scalar::Bool(v) => Value::scalar(Atom::Bool(*v), current),
                        Scalar::Number(v) => Value::scalar(Atom::Number(*v), current),
                        Scalar::String(v) => self.string(v, current)?,
                    }
                    .item()?;
                    found |= self.compare(&value, Relation::Eq, &item)?;
                }
                Value::scalar(Atom::Bool(found), current)
            }
            Matches { value, pattern } => {
                let value = self
                    .eval(value, current, &format!("{pointer}/value"))?
                    .string()?;
                self.scan(value.len())?;
                Value::scalar(
                    Atom::Bool(self.checked.patterns[pattern].is_match(&value)),
                    current,
                )
            }
            Concat { args } => {
                let mut output = std::string::String::new();
                for (index, arg) in args.iter().enumerate() {
                    let value = self
                        .eval(arg, current, &format!("{pointer}/args/{index}"))?
                        .string()?;
                    self.scan(value.len())?;
                    self.allocate(value.len())?;
                    output.push_str(&value);
                }
                self.string(&output, current)?
            }
            Replace { value, from, to } => {
                let value = self
                    .eval(value, current, &format!("{pointer}/value"))?
                    .string()?;
                self.scan(value.len().saturating_add(from.len()))?;
                let count = value.matches(from).count();
                let size = value
                    .len()
                    .saturating_sub(count.saturating_mul(from.len()))
                    .saturating_add(count.saturating_mul(to.len()));
                self.allocate(size)?;
                self.scan(value.len())?;
                let output = value.replace(from, to);
                self.string(&output, current)?
            }
        };
        if matches!(
            &value,
            Value::Item(Item {
                atom: Atom::Bool(false),
                ..
            })
        ) {
            self.observe(pointer, current);
        }
        Ok(value)
    }
}

#[cfg(test)]
#[path = "operators_v3_tests.rs"]
mod tests;
