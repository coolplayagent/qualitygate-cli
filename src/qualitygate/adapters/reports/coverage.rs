use super::*;
use anyhow::{Context, Result, bail};
use roxmltree::Node;
use std::collections::BTreeMap;

pub(super) fn lcov(text: &str) -> Result<Data> {
    let mut file = None;
    let mut records: BTreeMap<(String, usize), CoverageLine> = BTreeMap::new();
    for line in text.lines() {
        if let Some(path) = line.strip_prefix("SF:") {
            if file.is_some() || path.is_empty() {
                bail!("Malformed LCOV source record");
            }
            file = Some(path.to_string());
        } else if line == "end_of_record" {
            file = None;
        } else if let Some(values) = line.strip_prefix("DA:") {
            let file = file.as_ref().context("LCOV DA without SF")?;
            let values: Vec<_> = values.split(',').collect();
            if values.len() < 2 {
                bail!("Malformed LCOV DA");
            }
            let number: usize = values[0].parse()?;
            let record = records
                .entry((file.clone(), number))
                .or_insert(CoverageLine {
                    file: file.clone(),
                    line: number,
                    hits: 0,
                    branches_found: 0,
                    branches_hit: 0,
                });
            record.hits = record.hits.saturating_add(values[1].parse()?);
        } else if let Some(values) = line.strip_prefix("BRDA:") {
            let file = file.as_ref().context("LCOV BRDA without SF")?;
            let values: Vec<_> = values.split(',').collect();
            if values.len() != 4 {
                bail!("Malformed LCOV BRDA");
            }
            let number: usize = values[0].parse()?;
            let record = records
                .entry((file.clone(), number))
                .or_insert(CoverageLine {
                    file: file.clone(),
                    line: number,
                    hits: 0,
                    branches_found: 0,
                    branches_hit: 0,
                });
            record.branches_found += 1;
            if values[3] != "-" && values[3].parse::<u64>()? > 0 {
                record.branches_hit += 1;
            }
        }
    }
    if file.is_some() || records.is_empty() {
        bail!("LCOV report is incomplete or has no executable line records");
    }
    Ok(Data {
        coverage: records.into_values().collect(),
        ..Data::default()
    })
}

pub(super) fn xml(format: ReportFormat, root: Node<'_, '_>, data: &mut Data) -> Result<()> {
    use super::xml::number;
    for line in root.descendants().filter(|node| node.has_tag_name("line")) {
        if format == ReportFormat::Jacoco {
            let source = line
                .ancestors()
                .find(|node| node.has_tag_name("sourcefile"))
                .context("JaCoCo sourcefile missing")?;
            let package = source
                .parent()
                .and_then(|node| node.attribute("name"))
                .unwrap_or("");
            let name = source
                .attribute("name")
                .context("JaCoCo source file name missing")?;
            let file = if package.is_empty() {
                name.into()
            } else {
                format!("{package}/{name}")
            };
            let covered = number(line, "cb")?;
            data.coverage.push(CoverageLine {
                file,
                line: number(line, "nr")?,
                hits: number(line, "ci")? as u64,
                branches_found: covered + number(line, "mb")?,
                branches_hit: covered,
            });
        } else {
            // Class-level line records avoid double-counting per-method copies.
            if line.ancestors().any(|node| node.has_tag_name("method")) {
                continue;
            }
            let class = line
                .ancestors()
                .find(|node| node.has_tag_name("class"))
                .context("Cobertura class missing")?;
            let file = class
                .attribute("filename")
                .context("Cobertura filename missing")?;
            let mut record = CoverageLine {
                file: file.into(),
                line: number(line, "number")?,
                hits: number(line, "hits")? as u64,
                branches_found: 0,
                branches_hit: 0,
            };
            if line.attribute("branch") == Some("true") {
                let coverage = line
                    .attribute("condition-coverage")
                    .context("Branch coverage counts missing")?;
                let counts = coverage
                    .split_once('(')
                    .context("Invalid condition-coverage")?
                    .1
                    .trim_end_matches(')');
                let (hit, found) = counts
                    .split_once('/')
                    .context("Invalid condition-coverage counts")?;
                record.branches_hit = hit.parse()?;
                record.branches_found = found.parse()?;
            }
            data.coverage.push(record);
        }
    }
    if data.coverage.is_empty() {
        bail!("Coverage XML has no executable line records");
    }
    Ok(())
}
