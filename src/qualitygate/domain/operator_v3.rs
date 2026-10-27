//! Version 3 typed lambdas. Lexical variables and explicit, finite fact sources.
use super::operator::{Field, MAX_DEPTH, MAX_NODES, Relation, Scalar};
use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_SOURCES: usize = 16;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Expr {
    Bool {
        value: bool,
    },
    Number {
        value: i64,
    },
    String {
        value: String,
    },
    Field {
        name: Field,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        of: Option<Box<Expr>>,
    },
    Entities,
    Source {
        name: String,
    },
    Var {
        name: String,
    },
    And {
        args: Vec<Expr>,
    },
    Or {
        args: Vec<Expr>,
    },
    Not {
        arg: Box<Expr>,
    },
    All {
        input: Box<Expr>,
        #[serde(default, rename = "as", skip_serializing_if = "Option::is_none")]
        binding: Option<String>,
        predicate: Box<Expr>,
    },
    Any {
        input: Box<Expr>,
        #[serde(default, rename = "as", skip_serializing_if = "Option::is_none")]
        binding: Option<String>,
        predicate: Box<Expr>,
    },
    Filter {
        input: Box<Expr>,
        #[serde(default, rename = "as", skip_serializing_if = "Option::is_none")]
        binding: Option<String>,
        predicate: Box<Expr>,
    },
    Map {
        input: Box<Expr>,
        #[serde(rename = "as")]
        binding: String,
        body: Box<Expr>,
    },
    Count {
        input: Box<Expr>,
    },
    Distinct {
        input: Box<Expr>,
    },
    Contains {
        input: Box<Expr>,
        value: Box<Expr>,
    },
    Subset {
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Compare {
        left: Box<Expr>,
        relation: Relation,
        right: Box<Expr>,
    },
    In {
        value: Box<Expr>,
        set: Vec<Scalar>,
    },
    Matches {
        value: Box<Expr>,
        pattern: String,
    },
    Concat {
        args: Vec<Expr>,
    },
    Replace {
        value: Box<Expr>,
        from: String,
        to: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Kind {
    Bool,
    Number,
    String,
    Entity(String),
    Collection(Box<Kind>),
}
impl Kind {
    fn element(self) -> Result<Self, String> {
        if let Self::Collection(kind) = self {
            Ok(*kind)
        } else {
            Err("Expected a collection".into())
        }
    }
    fn scalar(&self) -> Result<(), String> {
        if matches!(self, Self::Bool | Self::Number | Self::String) {
            Ok(())
        } else {
            Err("Expected a scalar".into())
        }
    }
}

pub struct Validated {
    pub patterns: BTreeMap<String, Regex>,
    pub path_sources: BTreeSet<String>,
}
struct Checker<'a> {
    sources: &'a BTreeSet<String>,
    variables: BTreeMap<String, Kind>,
    checked: Validated,
    nodes: usize,
}

pub fn identifier(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name.len() > 64
        || !name.as_bytes()[0].is_ascii_alphabetic()
        || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return Err(
            "Names require 1..64 ASCII letters, digits or underscores, starting with a letter"
                .into(),
        );
    }
    Ok(())
}
fn expect(actual: Kind, expected: Kind) -> Result<(), String> {
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "Expression type mismatch: expected {expected:?}, got {actual:?}"
        ))
    }
}
fn literal(value: &str) -> Result<(), String> {
    if value.len() <= 4096 {
        Ok(())
    } else {
        Err("Expression literal exceeds 4096 UTF-8 bytes".into())
    }
}
fn list(len: usize) -> Result<(), String> {
    if (1..=64).contains(&len) {
        Ok(())
    } else {
        Err("Expression list requires 1..64 operands".into())
    }
}
fn scalar_kind(value: &Scalar) -> Result<Kind, String> {
    Ok(match value {
        Scalar::Bool(_) => Kind::Bool,
        Scalar::Number(_) => Kind::Number,
        Scalar::String(s) => {
            literal(s)?;
            Kind::String
        }
    })
}
impl Expr {
    /// Legacy rules retain their original parser, type checker and evaluator.
    pub fn legacy(&self) -> Result<super::operator::Expr, String> {
        serde_json::from_value(serde_json::to_value(self).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())
    }
    pub fn validate(&self, sources: &BTreeSet<String>) -> Result<Validated, String> {
        if sources.len() > MAX_SOURCES {
            return Err("Expression exceeds 16 sources".into());
        }
        for source in sources {
            identifier(source)?;
            if source == "entities" {
                return Err("Source name entities is reserved".into());
            }
        }
        let mut checker = Checker {
            sources,
            variables: BTreeMap::new(),
            nodes: 0,
            checked: Validated {
                patterns: BTreeMap::new(),
                path_sources: BTreeSet::new(),
            },
        };
        expect(checker.kind(self, 0, None)?, Kind::Bool)?;
        Ok(checker.checked)
    }
}
impl Checker<'_> {
    fn predicate(
        &mut self,
        input: &Expr,
        binding: Option<&String>,
        body: &Expr,
        depth: usize,
        current: Option<&Kind>,
    ) -> Result<(Kind, Kind), String> {
        let element = self.kind(input, depth, current)?.element()?;
        if let Some(name) = binding {
            identifier(name)?;
            if self.variables.contains_key(name) {
                return Err(format!("Variable shadowing is forbidden: {name}"));
            }
            self.variables.insert(name.clone(), element.clone());
        }
        let result = self.kind(body, depth, Some(&element));
        if let Some(name) = binding {
            self.variables.remove(name);
        }
        Ok((element, result?))
    }
    fn kind(&mut self, expr: &Expr, depth: usize, current: Option<&Kind>) -> Result<Kind, String> {
        self.nodes += 1;
        if depth > MAX_DEPTH || self.nodes > MAX_NODES {
            return Err("Expression exceeds depth 16 or 256 nodes".into());
        }
        let next = depth + 1;
        use Expr::*;
        match expr {
            Bool { .. } => Ok(Kind::Bool),
            Number { .. } => Ok(Kind::Number),
            String { value } => {
                literal(value)?;
                Ok(Kind::String)
            }
            Entities => Ok(Kind::Collection(Box::new(Kind::Entity("entities".into())))),
            Source { name } => {
                if !self.sources.contains(name) {
                    return Err(format!("Undeclared source: {name}"));
                }
                Ok(Kind::Collection(Box::new(Kind::Entity(name.clone()))))
            }
            Var { name } => self
                .variables
                .get(name)
                .cloned()
                .ok_or_else(|| format!("Unbound variable: {name}")),
            Field { name, of } => {
                let value = if let Some(of) = of {
                    self.kind(of, next, current)?
                } else {
                    current
                        .cloned()
                        .ok_or("Entity field requires a predicate binding")?
                };
                let Kind::Entity(source) = value else {
                    return Err("Fields require an entity".into());
                };
                if matches!(name, super::operator::Field::Path) {
                    self.checked.path_sources.insert(source);
                }
                Ok(Kind::String)
            }
            And { args } | Or { args } | Concat { args } => {
                list(args.len())?;
                let kind = if matches!(expr, Concat { .. }) {
                    Kind::String
                } else {
                    Kind::Bool
                };
                for arg in args {
                    expect(self.kind(arg, next, current)?, kind.clone())?;
                }
                Ok(kind)
            }
            Not { arg } => {
                expect(self.kind(arg, next, current)?, Kind::Bool)?;
                Ok(Kind::Bool)
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
                let (element, result) =
                    self.predicate(input, binding.as_ref(), predicate, next, current)?;
                expect(result, Kind::Bool)?;
                Ok(if matches!(expr, Filter { .. }) {
                    Kind::Collection(Box::new(element))
                } else {
                    Kind::Bool
                })
            }
            Map {
                input,
                binding,
                body,
            } => {
                let (_, result) = self.predicate(input, Some(binding), body, next, current)?;
                result.scalar()?;
                Ok(Kind::Collection(Box::new(result)))
            }
            Count { input } => {
                self.kind(input, next, current)?.element()?;
                Ok(Kind::Number)
            }
            Distinct { input } => {
                let element = self.kind(input, next, current)?.element()?;
                element.scalar()?;
                Ok(Kind::Collection(Box::new(element)))
            }
            Contains { input, value } => {
                let element = self.kind(input, next, current)?.element()?;
                element.scalar()?;
                expect(self.kind(value, next, current)?, element)?;
                Ok(Kind::Bool)
            }
            Subset { left, right } => {
                let element = self.kind(left, next, current)?.element()?;
                element.scalar()?;
                expect(self.kind(right, next, current)?.element()?, element)?;
                Ok(Kind::Bool)
            }
            Compare {
                left,
                relation,
                right,
            } => {
                let kind = self.kind(left, next, current)?;
                kind.scalar()?;
                expect(self.kind(right, next, current)?, kind.clone())?;
                if !matches!(relation, Relation::Eq | Relation::Ne) {
                    expect(kind, Kind::Number)?;
                }
                Ok(Kind::Bool)
            }
            In { value, set } => {
                let kind = self.kind(value, next, current)?;
                kind.scalar()?;
                list(set.len())?;
                for item in set {
                    expect(scalar_kind(item)?, kind.clone())?;
                }
                Ok(Kind::Bool)
            }
            Matches { value, pattern } => {
                expect(self.kind(value, next, current)?, Kind::String)?;
                if pattern.len() > 1024 {
                    return Err("Expression regex exceeds 1024 bytes".into());
                }
                if !self.checked.patterns.contains_key(pattern) {
                    let regex = RegexBuilder::new(pattern)
                        .size_limit(65_536)
                        .dfa_size_limit(262_144)
                        .build()
                        .map_err(|e| format!("Invalid bounded regex: {e}"))?;
                    self.checked.patterns.insert(pattern.clone(), regex);
                }
                Ok(Kind::Bool)
            }
            Replace { value, from, to } => {
                expect(self.kind(value, next, current)?, Kind::String)?;
                literal(from)?;
                literal(to)?;
                if from.is_empty() {
                    return Err("Replacement from must be nonempty".into());
                }
                Ok(Kind::String)
            }
        }
    }
}

#[cfg(test)]
#[path = "operator_v3_tests.rs"]
mod tests;
