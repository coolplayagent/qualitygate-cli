//! Closed source and scalar types for version-four tool-fact expressions.
use crate::tool_evidence::decimal::Decimal;
use serde::{Deserialize, Serialize};

pub use crate::tool_evidence::contracts::FactSchema;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dimension {
    pub schema: FactSchema,
    pub unit: String,
    pub measurement: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FactSource {
    pub producer: String,
    pub schema: FactSchema,
    pub schema_version: u32,
    pub project: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configuration: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metric: Option<String>,
    pub unit: String,
    pub measurement: String,
    #[serde(default = "minimum_lines")]
    pub minimum_lines: u32,
    #[serde(default)]
    pub minimum_nodes: u32,
    #[serde(default = "zero")]
    pub similarity_floor: Decimal,
}
fn minimum_lines() -> u32 {
    1
}
fn zero() -> Decimal {
    Decimal::from_i64(0)
}
impl FactSource {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 {
            return Err("Unsupported tool fact schema version".into());
        }
        for value in [&self.producer, &self.project, &self.unit, &self.measurement]
            .into_iter()
            .chain(self.target.iter())
            .chain(self.configuration.iter())
            .chain(self.metric.iter())
        {
            if value.trim().is_empty() || value.len() > 1024 || value.chars().any(char::is_control)
            {
                return Err("Fact source identity requires 1..1024 non-control UTF-8 bytes".into());
            }
        }
        if self.configuration.is_some() && self.target.is_none() {
            return Err("Fact source configuration requires an explicit target".into());
        }
        if (self.schema == FactSchema::FunctionMetric) != self.metric.is_some() {
            return Err("Metric sources require a metric selector; clone sources forbid it".into());
        }
        if self.schema == FactSchema::ClonePair && self.unit != "ratio" {
            return Err("Clone similarity requires ratio units".into());
        }
        self.similarity_floor
            .validate()
            .map_err(|error| error.to_string())?;
        if self.minimum_lines == 0
            || self.similarity_floor < zero()
            || self.similarity_floor > Decimal::from_i64(1)
        {
            return Err("Invalid clone source selection bounds".into());
        }
        if self.schema == FactSchema::FunctionMetric
            && (self.minimum_lines != 1
                || self.minimum_nodes != 0
                || self.similarity_floor != zero())
        {
            return Err("Metric sources cannot declare clone selection bounds".into());
        }
        Ok(())
    }
    pub fn source_type(&self) -> SourceType {
        let dimension = Dimension {
            schema: self.schema,
            unit: self.unit.clone(),
            measurement: self.measurement.clone(),
        };
        match self.schema {
            FactSchema::FunctionMetric => SourceType::Metric(dimension),
            FactSchema::ClonePair => SourceType::ClonePair(dimension),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Field {
    Name,
    Text,
    Path,
    Id,
    Project,
    Target,
    Configuration,
    Metric,
    Value,
    Unit,
    Measurement,
    LeftPath,
    RightPath,
    LeftSymbol,
    RightSymbol,
    Similarity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceType {
    Syntax,
    Metric(Dimension),
    ClonePair(Dimension),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScalarType {
    Bool,
    Number(Dimension),
    String,
}
impl SourceType {
    pub fn field(&self, field: Field) -> Result<ScalarType, String> {
        use Field::*;
        let result = match (self, field) {
            (Self::Syntax, Name | Text | Path) => ScalarType::String,
            (Self::Metric(dimension), Value) | (Self::ClonePair(dimension), Similarity) => {
                ScalarType::Number(dimension.clone())
            }
            (
                Self::Metric(_),
                Id | Project | Target | Configuration | Name | Path | Metric | Unit | Measurement,
            )
            | (
                Self::ClonePair(_),
                Id | Project | Target | Configuration | LeftPath | RightPath | LeftSymbol
                | RightSymbol | Unit | Measurement,
            ) => ScalarType::String,
            _ => return Err(format!("Field {field:?} is unavailable for {self:?}")),
        };
        Ok(result)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Scalar {
    Bool(bool),
    Number(i64),
    Decimal(Decimal),
    String(String),
}
