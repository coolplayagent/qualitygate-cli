//! coverage.py JSON v2/v3 carries an explicit branch-measurement flag.
use super::{CoverageLine, Data};
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Deserialize)]
struct Report {
    meta: Meta,
    #[serde(deserialize_with = "unique_files")]
    files: BTreeMap<String, File>,
    totals: Summary,
}

fn unique_files<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<BTreeMap<String, File>, D::Error> {
    struct Files;
    impl<'de> serde::de::Visitor<'de> for Files {
        type Value = BTreeMap<String, File>;
        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a unique coverage.py file inventory")
        }
        fn visit_map<A: serde::de::MapAccess<'de>>(
            self,
            mut map: A,
        ) -> std::result::Result<Self::Value, A::Error> {
            let mut files = BTreeMap::new();
            while let Some((name, file)) = map.next_entry()? {
                if files.insert(name, file).is_some() {
                    return Err(serde::de::Error::custom(
                        "Duplicate coverage.py source file",
                    ));
                }
            }
            Ok(files)
        }
    }
    deserializer.deserialize_map(Files)
}
#[derive(Deserialize)]
struct Meta {
    format: u32,
    version: String,
    branch_coverage: bool,
}
#[derive(Deserialize)]
struct File {
    executed_lines: Vec<usize>,
    missing_lines: Vec<usize>,
    excluded_lines: Vec<usize>,
    summary: Summary,
    executed_branches: Option<Vec<[i64; 2]>>,
    missing_branches: Option<Vec<[i64; 2]>>,
}
#[derive(Deserialize)]
struct Summary {
    covered_lines: usize,
    num_statements: usize,
    missing_lines: usize,
    excluded_lines: usize,
    num_branches: Option<usize>,
    num_partial_branches: Option<usize>,
    covered_branches: Option<usize>,
    missing_branches: Option<usize>,
}

type Counts = BTreeMap<&'static str, usize>;
impl Summary {
    fn validate(&self, observed: &Counts, branches: bool) -> Result<()> {
        for (name, value) in [
            ("covered_lines", self.covered_lines),
            ("num_statements", self.num_statements),
            ("missing_lines", self.missing_lines),
            ("excluded_lines", self.excluded_lines),
        ] {
            if observed.get(name).copied().unwrap_or_default() != value {
                bail!("coverage.py {name} summary contradicts line records");
            }
        }
        for (name, value) in [
            ("num_branches", self.num_branches),
            ("num_partial_branches", self.num_partial_branches),
            ("covered_branches", self.covered_branches),
            ("missing_branches", self.missing_branches),
        ] {
            if branches && value != Some(observed.get(name).copied().unwrap_or_default()) {
                bail!("coverage.py {name} summary contradicts branch records");
            }
            if !branches && value.is_some() {
                bail!("coverage.py branch counters contradict disabled measurement");
            }
        }
        Ok(())
    }
}

fn lines(values: &[usize]) -> Result<BTreeSet<usize>> {
    let set: BTreeSet<_> = values.iter().copied().collect();
    if set.contains(&0) || set.len() != values.len() {
        bail!("Invalid or duplicate coverage.py line number");
    }
    Ok(set)
}

fn branches(
    values: Option<Vec<[i64; 2]>>,
    enabled: bool,
    maximum: usize,
) -> Result<BTreeSet<(usize, i64)>> {
    if !enabled {
        if values.is_some() {
            bail!("coverage.py branch records contradict disabled measurement");
        }
        return Ok(BTreeSet::new());
    }
    let mut result = BTreeSet::new();
    for [from, to] in values.context("coverage.py branch record inventory is missing")? {
        let from = usize::try_from(from)?;
        let target = to
            .checked_abs()
            .and_then(|value| usize::try_from(value).ok())
            .context("Invalid coverage.py branch destination")?;
        if from == 0 || from > maximum || to == 0 || target > maximum || !result.insert((from, to))
        {
            bail!("Invalid or duplicate coverage.py branch record");
        }
    }
    Ok(result)
}

pub(super) fn parse(text: &str) -> Result<Data> {
    let report: Report = serde_json::from_str(text)?;
    if ![2, 3].contains(&report.meta.format)
        || report.meta.version.trim().is_empty()
        || report.meta.version.contains('\0')
    {
        bail!("Unsupported coverage.py format or missing producer version");
    }
    if report.files.is_empty() {
        bail!("coverage.py report has no source inventory");
    }
    let mut data = Data {
        branch_coverage: report.meta.branch_coverage,
        coverage_producer: Some(
            serde_json::json!({"tool":"coverage.py", "version":report.meta.version,
            "format":report.meta.format, "branch_coverage":report.meta.branch_coverage}),
        ),
        ..Data::default()
    };
    let mut totals = Counts::new();
    let mut budget = 16 * 1024 * 1024;
    for (file, records) in report.files {
        super::coverage_budget(&mut budget, &file)?;
        data.coverage_files.push(file.clone());
        // CPython can trace an empty module at synthetic line zero in line-only
        // mode. It is inventory evidence, never an executable source line.
        let executed = if !data.branch_coverage
            && records.executed_lines == [0]
            && records.missing_lines.is_empty()
            && records.excluded_lines.is_empty()
            && records.summary.num_statements == 0
        {
            BTreeSet::new()
        } else {
            lines(&records.executed_lines)?
        };
        let missing = lines(&records.missing_lines)?;
        let excluded = lines(&records.excluded_lines)?;
        if !executed.is_disjoint(&missing) || !missing.is_disjoint(&excluded) {
            bail!("coverage.py line inventories contradict each other");
        }
        let hit: BTreeSet<_> = executed.difference(&excluded).copied().collect();
        let mut source: BTreeMap<_, _> = hit
            .iter()
            .chain(&missing)
            .chain(&excluded)
            .map(|line| (*line, (0usize, 0usize)))
            .collect();
        let maximum = source.keys().next_back().copied().unwrap_or_default();
        let executed_branches = branches(records.executed_branches, data.branch_coverage, maximum)?;
        let missing_branches = branches(records.missing_branches, data.branch_coverage, maximum)?;
        if !executed_branches.is_disjoint(&missing_branches) {
            bail!("coverage.py branch inventories contradict each other");
        }
        for (arcs, covered) in [(&executed_branches, true), (&missing_branches, false)] {
            for (line, _) in arcs {
                if excluded.contains(line) || (covered && !hit.contains(line)) {
                    bail!("coverage.py branch source contradicts line coverage");
                }
                let counts = source
                    .get_mut(line)
                    .context("coverage.py branch has no source line")?;
                counts.0 += 1;
                counts.1 += usize::from(covered);
            }
        }
        let observed: Counts = [
            ("covered_lines", hit.len()),
            ("num_statements", hit.len() + missing.len()),
            ("missing_lines", missing.len()),
            ("excluded_lines", excluded.len()),
            (
                "num_branches",
                executed_branches.len() + missing_branches.len(),
            ),
            ("covered_branches", executed_branches.len()),
            ("missing_branches", missing_branches.len()),
            (
                "num_partial_branches",
                missing_branches
                    .iter()
                    .filter(|(line, _)| hit.contains(line))
                    .count(),
            ),
        ]
        .into();
        records.summary.validate(&observed, data.branch_coverage)?;
        for (name, value) in observed {
            let total = totals.entry(name).or_default();
            *total = total
                .checked_add(value)
                .context("coverage.py total counter overflow")?;
        }
        for (line, (found, covered)) in source {
            super::coverage_budget(&mut budget, &file)?;
            data.coverage.push(CoverageLine {
                file: file.clone(),
                line,
                hits: u64::from(hit.contains(&line)),
                branches_found: found,
                branches_hit: covered,
                excluded: excluded.contains(&line),
            });
        }
    }
    report.totals.validate(&totals, data.branch_coverage)?;
    Ok(data)
}
