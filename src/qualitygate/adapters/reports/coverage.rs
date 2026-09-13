//! Normalize Cobertura class records and validate available summary counts.
use super::*;
use anyhow::{Context, Result, bail};
use roxmltree::Node;
use std::collections::BTreeSet;

pub(super) fn xml(format: ReportFormat, root: Node<'_, '_>, data: &mut Data) -> Result<()> {
    if format == ReportFormat::Jacoco {
        return super::jacoco::parse(root, data);
    }
    use super::xml::{number, optional_number};
    // Zero counters also occur when coverage.py branch measurement is disabled.
    data.branch_coverage = optional_number(root, "branches-valid")?.is_some_and(|count| count > 0);
    let mut budget = 16 * 1024 * 1024;
    for source in root
        .children()
        .filter(|node| node.has_tag_name("sources"))
        .flat_map(|sources| {
            sources
                .children()
                .filter(|node| node.has_tag_name("source"))
        })
    {
        let source = source
            .text()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .context("Cobertura source root is empty")?;
        if data.coverage_roots.len() == 64 {
            bail!("Coverage source roots exceed 64 entries");
        }
        super::coverage_budget(&mut budget, source)?;
        data.coverage_roots.push(source.to_owned());
    }
    let mut inventory = BTreeSet::new();
    let mut records = BTreeSet::new();
    for class in root.descendants().filter(|node| node.has_tag_name("class")) {
        let file = class
            .attribute("filename")
            .filter(|file| !file.is_empty())
            .context("Cobertura source file name missing")?;
        super::coverage_budget(&mut budget, file)?;
        inventory.insert(file.to_owned());
        for line in class
            .children()
            .filter(|node| node.has_tag_name("lines"))
            .flat_map(|lines| lines.children().filter(|node| node.has_tag_name("line")))
        {
            super::coverage_budget(&mut budget, file)?;
            let mut record = CoverageLine {
                excluded: false,
                file: file.into(),
                line: number(line, "number")?,
                hits: number(line, "hits")? as u64,
                branches_found: 0,
                branches_hit: 0,
            };
            if record.line == 0 || !records.insert((file, record.line)) {
                bail!("Invalid or duplicate Cobertura source line");
            }
            match line.attribute("branch") {
                None | Some("false") => {
                    if line.attribute("condition-coverage").is_some() {
                        bail!("Cobertura branch counters lack a true branch flag");
                    }
                }
                Some("true") => {
                    let coverage = line
                        .attribute("condition-coverage")
                        .context("Branch coverage counts missing")?;
                    let (rate, counts) = coverage
                        .split_once("% (")
                        .context("Invalid condition-coverage")?;
                    let rate = rate.parse::<f64>()?;
                    let (hit, found) = counts
                        .strip_suffix(')')
                        .context("Unclosed condition-coverage")?
                        .split_once('/')
                        .context("Invalid condition-coverage counts")?;
                    record.branches_hit = hit.parse()?;
                    record.branches_found = found.parse()?;
                    if !rate.is_finite()
                        || !(0.0..=100.0).contains(&rate)
                        || record.branches_found == 0
                        || record.branches_hit > record.branches_found
                        || (record.hits == 0 && record.branches_hit > 0)
                    {
                        bail!("Invalid Cobertura branch counters");
                    }
                    let measured =
                        100.0 * record.branches_hit as f64 / record.branches_found as f64;
                    // coverage.py truncates percentages; counts determine the gate.
                    if (rate - measured).abs() >= 1.0 {
                        bail!("Cobertura branch percentage contradicts counts");
                    }
                }
                Some(_) => bail!("Invalid Cobertura branch flag"),
            }
            data.coverage.push(record);
        }
    }
    data.coverage_files = inventory.into_iter().collect();
    if data.coverage_files.is_empty() {
        bail!("Coverage XML has no source file inventory");
    }
    for line in root.descendants().filter(|node| node.has_tag_name("line")) {
        if line.ancestors().any(|node| node.has_tag_name("method")) {
            continue;
        }
        if !line.parent().is_some_and(|parent| {
            parent.has_tag_name("lines")
                && parent
                    .parent()
                    .is_some_and(|class| class.has_tag_name("class"))
        }) {
            bail!("Cobertura line is outside class-level line records");
        }
    }
    for (name, observed) in [
        ("lines-valid", data.coverage.len()),
        (
            "lines-covered",
            data.coverage.iter().filter(|line| line.hits > 0).count(),
        ),
        (
            "branches-valid",
            data.coverage
                .iter()
                .try_fold(0usize, |sum, line| sum.checked_add(line.branches_found))
                .context("Branch count overflow")?,
        ),
        (
            "branches-covered",
            data.coverage
                .iter()
                .try_fold(0usize, |sum, line| sum.checked_add(line.branches_hit))
                .context("Branch count overflow")?,
        ),
    ] {
        if let Some(value) = root.attribute(name)
            && value.parse::<usize>()? != observed
        {
            bail!("Cobertura {name} does not match source records");
        }
    }
    Ok(())
}
