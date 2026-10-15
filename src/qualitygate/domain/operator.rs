//! Typed, finite expression contracts. No user code, dynamic field access or I/O.
#[cfg(test)]
#[path = "operator_tests.rs"]
mod tests;
use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const MAX_NODES: usize = 256;
pub const MAX_DEPTH: usize = 16;
pub const MAX_ENTITIES: usize = 50_000;
pub const MAX_FACT_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_STEPS: usize = 100_000;
pub const MAX_SCAN_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_SECONDS: u64 = 30;

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
    },
    Entities,
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
        predicate: Box<Expr>,
    },
    Any {
        input: Box<Expr>,
        predicate: Box<Expr>,
    },
    Filter {
        input: Box<Expr>,
        predicate: Box<Expr>,
    },
    Count {
        input: Box<Expr>,
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
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Field {
    Name,
    Text,
    Path,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Relation {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Scalar {
    Bool(bool),
    Number(i64),
    String(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Bool,
    Number,
    String,
    Collection,
}

pub struct Validated {
    pub patterns: BTreeMap<String, Regex>,
    pub requires_path: bool,
}

impl Expr {
    pub fn validate(&self) -> Result<Validated, String> {
        let mut checked = Validated {
            patterns: BTreeMap::new(),
            requires_path: false,
        };
        expect(self.kind(0, false, &mut 0, &mut checked)?, Kind::Bool)?;
        Ok(checked)
    }

    fn kind(
        &self,
        depth: usize,
        bound: bool,
        nodes: &mut usize,
        checked: &mut Validated,
    ) -> Result<Kind, String> {
        *nodes += 1;
        if depth > MAX_DEPTH || *nodes > MAX_NODES {
            return Err("Expression exceeds depth 16 or 256 nodes".into());
        }
        let next = depth + 1;
        match self {
            Self::Bool { .. } => Ok(Kind::Bool),
            Self::Number { .. } => Ok(Kind::Number),
            Self::String { value } => {
                literal(value)?;
                Ok(Kind::String)
            }
            Self::Field { name } => {
                if !bound {
                    return Err("Entity fields require an all, any or filter predicate".into());
                }
                checked.requires_path |= matches!(name, Field::Path);
                Ok(Kind::String)
            }
            Self::Entities => Ok(Kind::Collection),
            Self::And { args } | Self::Or { args } => {
                if args.is_empty() || args.len() > 64 {
                    return Err("and/or require 1..64 operands".into());
                }
                for arg in args {
                    expect(arg.kind(next, bound, nodes, checked)?, Kind::Bool)?;
                }
                Ok(Kind::Bool)
            }
            Self::Not { arg } => {
                expect(arg.kind(next, bound, nodes, checked)?, Kind::Bool)?;
                Ok(Kind::Bool)
            }
            Self::All { input, predicate }
            | Self::Any { input, predicate }
            | Self::Filter { input, predicate } => {
                expect(input.kind(next, bound, nodes, checked)?, Kind::Collection)?;
                expect(predicate.kind(next, true, nodes, checked)?, Kind::Bool)?;
                Ok(if matches!(self, Self::Filter { .. }) {
                    Kind::Collection
                } else {
                    Kind::Bool
                })
            }
            Self::Count { input } => {
                expect(input.kind(next, bound, nodes, checked)?, Kind::Collection)?;
                Ok(Kind::Number)
            }
            Self::Compare {
                left,
                relation,
                right,
            } => {
                let kind = left.kind(next, bound, nodes, checked)?;
                expect(right.kind(next, bound, nodes, checked)?, kind)?;
                if kind == Kind::Collection
                    || (!matches!(relation, Relation::Eq | Relation::Ne) && kind != Kind::Number)
                {
                    return Err("Equality requires scalars; ordering requires integers".into());
                }
                Ok(Kind::Bool)
            }
            Self::In { value, set } => {
                let kind = value.kind(next, bound, nodes, checked)?;
                if set.is_empty() || set.len() > 64 {
                    return Err("Membership requires 1..64 scalar values".into());
                }
                for item in set {
                    let item_kind = match item {
                        Scalar::Bool(_) => Kind::Bool,
                        Scalar::Number(_) => Kind::Number,
                        Scalar::String(s) => {
                            literal(s)?;
                            Kind::String
                        }
                    };
                    expect(item_kind, kind)?;
                }
                Ok(Kind::Bool)
            }
            Self::Matches { value, pattern } => {
                expect(value.kind(next, bound, nodes, checked)?, Kind::String)?;
                if pattern.len() > 1024 {
                    return Err("Expression regex exceeds 1024 bytes".into());
                }
                if !checked.patterns.contains_key(pattern) {
                    let regex = RegexBuilder::new(pattern)
                        .size_limit(65_536)
                        .dfa_size_limit(262_144)
                        .build()
                        .map_err(|e| format!("Invalid bounded regex: {e}"))?;
                    checked.patterns.insert(pattern.clone(), regex);
                }
                Ok(Kind::Bool)
            }
        }
    }
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
        Err("Expression literal exceeds 4096 bytes".into())
    }
}
