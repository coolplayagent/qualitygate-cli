//! Validate JaCoCo source counters and their package/group/report rollups.
use super::{
    CoverageLine, Data,
    xml::{number, optional_number},
};
use anyhow::{Context, Result, bail};
use roxmltree::{Node, NodeId};
use std::collections::{BTreeMap, BTreeSet, HashMap};

#[derive(Clone, Copy, Default)]
struct Counts {
    instruction: (usize, usize),
    line: (usize, usize),
    branch: (usize, usize),
}

fn add(left: &mut (usize, usize), right: (usize, usize)) -> Result<()> {
    left.0 = left
        .0
        .checked_add(right.0)
        .context("JaCoCo missed count overflow")?;
    left.1 = left
        .1
        .checked_add(right.1)
        .context("JaCoCo covered count overflow")?;
    Ok(())
}

impl Counts {
    fn add(&mut self, other: Self) -> Result<()> {
        add(&mut self.instruction, other.instruction)?;
        add(&mut self.line, other.line)?;
        add(&mut self.branch, other.branch)
    }
    fn validate(self, node: Node<'_, '_>) -> Result<()> {
        let mut counters = BTreeMap::new();
        for counter in node.children().filter(|node| node.has_tag_name("counter")) {
            let kind = counter
                .attribute("type")
                .context("JaCoCo counter type missing")?;
            if ![
                "INSTRUCTION",
                "LINE",
                "BRANCH",
                "COMPLEXITY",
                "METHOD",
                "CLASS",
            ]
            .contains(&kind)
            {
                bail!("Unknown JaCoCo counter type");
            }
            if counters
                .insert(
                    kind,
                    (number(counter, "missed")?, number(counter, "covered")?),
                )
                .is_some()
            {
                bail!("Duplicate JaCoCo summary counter");
            }
        }
        for (kind, observed) in [
            ("INSTRUCTION", self.instruction),
            ("LINE", self.line),
            ("BRANCH", self.branch),
        ] {
            if counters.get(kind).copied().unwrap_or_default() != observed {
                bail!(
                    "JaCoCo {kind} summary does not match mapped source records at {}",
                    node.tag_name().name()
                );
            }
        }
        Ok(())
    }
}

pub(super) fn parse(root: Node<'_, '_>, data: &mut Data) -> Result<()> {
    data.branch_coverage = true;
    let mut totals: HashMap<NodeId, Counts> = HashMap::new();
    let mut inventory = BTreeSet::new();
    let mut budget = 16 * 1024 * 1024;
    for source in root
        .descendants()
        .filter(|node| node.has_tag_name("sourcefile"))
    {
        let package = source
            .parent()
            .filter(|node| node.has_tag_name("package"))
            .context("JaCoCo sourcefile must belong to a package")?;
        let package_name = package
            .attribute("name")
            .context("JaCoCo package name missing")?;
        let name = source
            .attribute("name")
            .filter(|name| !name.is_empty())
            .context("JaCoCo source file name missing")?;
        if package_name.len() + name.len() > 16 * 1024 {
            bail!("JaCoCo source path exceeds 16 KiB");
        }
        let file = if package_name.is_empty() {
            name.to_owned()
        } else {
            format!("{package_name}/{name}")
        };
        super::coverage_budget(&mut budget, &file)?;
        if !inventory.insert(file.clone()) {
            bail!("Ambiguous JaCoCo source file across packages/groups: {file}");
        }
        data.coverage_files.push(file.clone());
        let mut counts = Counts::default();
        let mut seen = BTreeSet::new();
        for line in source.children().filter(|node| node.has_tag_name("line")) {
            super::coverage_budget(&mut budget, &file)?;
            let line_number = number(line, "nr")?;
            if line_number == 0 || !seen.insert(line_number) {
                bail!("Invalid or duplicate JaCoCo source line");
            }
            let instruction = (
                optional_number(line, "mi")?.unwrap_or_default(),
                optional_number(line, "ci")?.unwrap_or_default(),
            );
            let branch = (
                optional_number(line, "mb")?.unwrap_or_default(),
                optional_number(line, "cb")?.unwrap_or_default(),
            );
            if instruction == (0, 0) {
                bail!("JaCoCo line has no executable instruction evidence");
            }
            if instruction.1 == 0 && branch.1 > 0 {
                bail!("JaCoCo covered branches contradict missed instructions");
            }
            counts.add(Counts {
                instruction,
                line: (
                    usize::from(instruction.1 == 0),
                    usize::from(instruction.1 > 0),
                ),
                branch,
            })?;
            data.coverage.push(CoverageLine {
                excluded: false,
                file: file.clone(),
                line: line_number,
                hits: instruction.1 as u64,
                branches_found: branch
                    .0
                    .checked_add(branch.1)
                    .context("JaCoCo branch count overflow")?,
                branches_hit: branch.1,
            });
        }
        counts.validate(source)?;
        let mut ancestor = Some(package);
        let mut depth = 0;
        while let Some(node) = ancestor {
            if !["report", "package", "group"]
                .iter()
                .any(|tag| node.has_tag_name(*tag))
            {
                bail!("Invalid JaCoCo coverage hierarchy");
            }
            depth += 1;
            if depth > 32 {
                bail!("JaCoCo coverage hierarchy exceeds depth 32");
            }
            totals.entry(node.id()).or_default().add(counts)?;
            ancestor = node.parent().filter(|parent| parent.is_element());
        }
    }
    if data.coverage_files.is_empty() {
        bail!("JaCoCo report has no source file inventory/debug information");
    }
    for line in root.descendants().filter(|node| node.has_tag_name("line")) {
        if !line
            .parent()
            .is_some_and(|parent| parent.has_tag_name("sourcefile"))
        {
            bail!("JaCoCo line is outside a sourcefile");
        }
    }
    for node in root.descendants().filter(|node| {
        ["report", "package", "group"]
            .iter()
            .any(|tag| node.has_tag_name(*tag))
    }) {
        totals
            .get(&node.id())
            .copied()
            .unwrap_or_default()
            .validate(node)?;
    }
    Ok(())
}
