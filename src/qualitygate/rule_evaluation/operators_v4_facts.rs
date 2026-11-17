//! Closed typed projections; this module never acquires producer evidence.
use crate::rule_catalog::operator_v4::{Field, Scalar, ScalarType, SourceType};
use crate::tool_evidence::contracts::{FactRecord, Location, Scope};
use anyhow::{Result, ensure};
use std::collections::{BTreeMap, BTreeSet};

pub struct Record {
    pub fields: BTreeMap<Field, Scalar>,
    pub locations: Vec<Location>,
}
pub struct Collection {
    pub records: Vec<Record>,
    pub complete: bool,
    pub source_type: SourceType,
}
impl Record {
    pub fn tool_bytes(record: &FactRecord, scope: &Scope) -> usize {
        let mut bytes = 2048usize
            .saturating_add(scope.project.len())
            .saturating_add(scope.target.as_ref().map_or(0, String::len))
            .saturating_add(scope.configuration.as_ref().map_or(0, String::len));
        let (id, unit, measurement, locations, metric) = match record {
            FactRecord::Metric(record) => (
                &record.id,
                &record.unit,
                &record.measurement,
                vec![&record.location],
                record.metric.len(),
            ),
            FactRecord::ClonePair(record) => (
                &record.id,
                &record.unit,
                &record.measurement,
                vec![&record.left, &record.right],
                0,
            ),
        };
        bytes = bytes
            .saturating_add(id.len())
            .saturating_add(unit.len())
            .saturating_add(measurement.len())
            .saturating_add(metric);
        for location in locations {
            bytes = bytes.saturating_add(
                location
                    .file
                    .len()
                    .saturating_add(location.symbol.len())
                    .saturating_mul(2),
            );
        }
        bytes
    }
    pub fn syntax(
        name: &str,
        text: &str,
        path: Option<&str>,
        range: Option<&crate::domain::Range>,
    ) -> Self {
        let mut fields = BTreeMap::from([
            (Field::Name, Scalar::String(name.into())),
            (Field::Text, Scalar::String(text.into())),
        ]);
        if let Some(path) = path {
            fields.insert(Field::Path, Scalar::String(path.into()));
        }
        let locations = path
            .zip(range)
            .map(|(file, range)| Location {
                file: file.into(),
                start_line: range.start_line,
                end_line: range.end_line,
                symbol: name.into(),
            })
            .into_iter()
            .collect();
        Self { fields, locations }
    }
    pub fn tool(record: &FactRecord, scope: &Scope) -> Self {
        let mut fields = BTreeMap::from([(Field::Project, Scalar::String(scope.project.clone()))]);
        if let Some(target) = &scope.target {
            fields.insert(Field::Target, Scalar::String(target.clone()));
        }
        if let Some(configuration) = &scope.configuration {
            fields.insert(Field::Configuration, Scalar::String(configuration.clone()));
        }
        let locations = match record {
            FactRecord::Metric(metric) => {
                fields.extend([
                    (Field::Id, Scalar::String(metric.id.clone())),
                    (Field::Name, Scalar::String(metric.location.symbol.clone())),
                    (Field::Path, Scalar::String(metric.location.file.clone())),
                    (Field::Metric, Scalar::String(metric.metric.clone())),
                    (Field::Value, Scalar::Decimal(metric.value.clone())),
                    (Field::Unit, Scalar::String(metric.unit.clone())),
                    (
                        Field::Measurement,
                        Scalar::String(metric.measurement.clone()),
                    ),
                ]);
                vec![metric.location.clone()]
            }
            FactRecord::ClonePair(pair) => {
                fields.extend([
                    (Field::Id, Scalar::String(pair.id.clone())),
                    (Field::LeftPath, Scalar::String(pair.left.file.clone())),
                    (Field::RightPath, Scalar::String(pair.right.file.clone())),
                    (Field::LeftSymbol, Scalar::String(pair.left.symbol.clone())),
                    (
                        Field::RightSymbol,
                        Scalar::String(pair.right.symbol.clone()),
                    ),
                    (Field::Similarity, Scalar::Decimal(pair.similarity.clone())),
                    (Field::Unit, Scalar::String(pair.unit.clone())),
                    (Field::Measurement, Scalar::String(pair.measurement.clone())),
                ]);
                vec![pair.left.clone(), pair.right.clone()]
            }
        };
        Self { fields, locations }
    }
    pub fn validate(
        &self,
        source_type: &SourceType,
        required: Option<&BTreeSet<Field>>,
    ) -> Result<()> {
        if let SourceType::Metric(dimension) | SourceType::ClonePair(dimension) = source_type {
            for (field, expected) in [
                (Field::Unit, &dimension.unit),
                (Field::Measurement, &dimension.measurement),
            ] {
                ensure!(
                    matches!(self.fields.get(&field),Some(Scalar::String(actual)) if actual==expected),
                    "Record measurement metadata differs from its typed source"
                );
            }
        }
        if let Some(required) = required {
            for field in required {
                ensure!(
                    self.fields.contains_key(field),
                    "Required field {field:?} unavailable"
                );
            }
        }
        for (field, value) in &self.fields {
            let expected = source_type.field(*field).map_err(anyhow::Error::msg)?;
            ensure!(
                matches!(
                    (&expected, value),
                    (ScalarType::Bool, Scalar::Bool(_))
                        | (ScalarType::String, Scalar::String(_))
                        | (
                            ScalarType::Number(_),
                            Scalar::Number(_) | Scalar::Decimal(_)
                        )
                ),
                "Invalid type for {field:?}"
            );
            if let Scalar::Decimal(value) = value {
                value.validate()?;
            }
        }
        Ok(())
    }
    pub fn bytes(&self) -> usize {
        self.fields
            .iter()
            .fold(0usize, |bytes, (field, value)| {
                bytes
                    .saturating_add(std::mem::size_of_val(field) + 64)
                    .saturating_add(match value {
                        Scalar::String(s) => s.len(),
                        _ => 64,
                    })
            })
            .saturating_add(
                self.locations
                    .iter()
                    .map(|location| {
                        location
                            .file
                            .len()
                            .saturating_add(location.symbol.len())
                            .saturating_add(128)
                    })
                    .sum::<usize>(),
            )
    }
}
