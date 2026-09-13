//! LCOV sections are validated independently, then merged by line/branch identity.

use super::{CoverageLine, Data};
use anyhow::{Context, Result, bail};
use std::collections::BTreeMap;

#[derive(Default)]
struct Section {
    file: String,
    lines: BTreeMap<usize, u64>,
    branches: BTreeMap<(usize, String, String), bool>,
    counters: BTreeMap<String, usize>,
}

pub(super) fn parse(text: &str) -> Result<Data> {
    let mut current: Option<Section> = None;
    let mut data = Data {
        branch_coverage: true,
        ..Data::default()
    };
    let mut lines: BTreeMap<(String, usize), u64> = BTreeMap::new();
    let mut branches = BTreeMap::new();
    let mut budget = 16 * 1024 * 1024;
    for line in text.lines() {
        if let Some(file) = line.strip_prefix("SF:") {
            if current.is_some() || file.is_empty() {
                bail!("Malformed LCOV source record");
            }
            super::coverage_budget(&mut budget, file)?;
            current = Some(Section {
                file: file.into(),
                ..Section::default()
            });
        } else if line == "end_of_record" {
            let section = current.take().context("LCOV end_of_record without SF")?;
            section.validate()?;
            data.branch_coverage &=
                section.counters.contains_key("BRF") && section.counters.contains_key("BRH");
            data.coverage_files.push(section.file.clone());
            for (line, hits) in section.lines {
                super::coverage_budget(&mut budget, &section.file)?;
                let total = lines.entry((section.file.clone(), line)).or_default();
                *total = total.saturating_add(hits);
            }
            for ((line, block, branch), hit) in section.branches {
                super::coverage_budget(&mut budget, &section.file)?;
                *branches
                    .entry((section.file.clone(), line, block, branch))
                    .or_insert(false) |= hit;
            }
        } else if let Some(values) = line.strip_prefix("DA:") {
            let section = current.as_mut().context("LCOV DA without SF")?;
            let values: Vec<_> = values.split(',').collect();
            if !(2..=3).contains(&values.len()) {
                bail!("Malformed LCOV DA");
            }
            if section
                .lines
                .insert(values[0].parse()?, values[1].parse()?)
                .is_some()
            {
                bail!("Duplicate LCOV DA within a source section");
            }
        } else if let Some(values) = line.strip_prefix("BRDA:") {
            let section = current.as_mut().context("LCOV BRDA without SF")?;
            let values: Vec<_> = values.split(',').collect();
            if values.len() != 4 || values[1].is_empty() || values[2].is_empty() {
                bail!("Malformed LCOV BRDA");
            }
            let hit = values[3] != "-" && values[3].parse::<u64>()? > 0;
            if section
                .branches
                .insert(
                    (values[0].parse()?, values[1].into(), values[2].into()),
                    hit,
                )
                .is_some()
            {
                bail!("Duplicate LCOV branch within a source section");
            }
        } else if let Some((key, value)) = line.split_once(':')
            && ["LF", "LH", "BRF", "BRH"].contains(&key)
        {
            let section = current.as_mut().context("LCOV counters without SF")?;
            if section
                .counters
                .insert(key.into(), value.parse()?)
                .is_some()
            {
                bail!("Duplicate LCOV summary counter");
            }
        }
    }
    if current.is_some() || data.coverage_files.is_empty() {
        bail!("LCOV report is incomplete or has no source records");
    }
    // Exporters can emit BRF:0/BRH:0 without branch instrumentation. Positive
    // records establish measurement; zero-only summaries cannot prove it.
    data.branch_coverage &= !branches.is_empty();
    let mut records: BTreeMap<_, _> = lines
        .into_iter()
        .map(|((file, line), hits)| {
            (
                (file.clone(), line),
                CoverageLine {
                    excluded: false,
                    file,
                    line,
                    hits,
                    branches_found: 0,
                    branches_hit: 0,
                },
            )
        })
        .collect();
    for ((file, line, _, _), hit) in branches {
        let record = records
            .get_mut(&(file, line))
            .context("LCOV branch has no executable line")?;
        record.branches_found += 1;
        record.branches_hit += usize::from(hit);
    }
    data.coverage = records.into_values().collect();
    Ok(data)
}

impl Section {
    fn validate(&self) -> Result<()> {
        if self.lines.is_empty() && self.counters.get("LF") != Some(&0) {
            bail!("Empty LCOV source requires explicit LF:0 inventory evidence");
        }
        for (key, observed) in [
            ("LF", self.lines.len()),
            ("LH", self.lines.values().filter(|hits| **hits > 0).count()),
            ("BRF", self.branches.len()),
            ("BRH", self.branches.values().filter(|hit| **hit).count()),
        ] {
            if self
                .counters
                .get(key)
                .is_some_and(|expected| *expected != observed)
            {
                bail!("LCOV {key} does not match source records");
            }
        }
        if self
            .branches
            .keys()
            .any(|(line, _, _)| !self.lines.contains_key(line))
        {
            bail!("LCOV branch has no executable line");
        }
        Ok(())
    }
}
