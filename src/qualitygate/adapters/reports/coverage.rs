use super::*;
use anyhow::{Context, Result, bail};
use roxmltree::Node;

pub(super) fn xml(format: ReportFormat, root: Node<'_, '_>, data: &mut Data) -> Result<()> {
    use super::xml::number;
    data.branch_coverage =
        format == ReportFormat::Jacoco || root.attribute("branches-valid").is_some();
    for source in root.descendants().filter(|node| {
        node.has_tag_name(if format == ReportFormat::Jacoco {
            "sourcefile"
        } else {
            "class"
        })
    }) {
        let name = source
            .attribute(if format == ReportFormat::Jacoco {
                "name"
            } else {
                "filename"
            })
            .context("Coverage source file name missing")?;
        let package = if format == ReportFormat::Jacoco {
            source
                .parent()
                .and_then(|parent| parent.attribute("name"))
                .unwrap_or("")
        } else {
            ""
        };
        data.coverage_files.push(if package.is_empty() {
            name.into()
        } else {
            format!("{package}/{name}")
        });
    }
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
                branches_found: covered
                    .checked_add(number(line, "mb")?)
                    .context("JaCoCo branch count overflow")?,
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
    if data.coverage_files.is_empty() {
        bail!("Coverage XML has no source file inventory");
    }
    if format == ReportFormat::Cobertura {
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
    }
    Ok(())
}
