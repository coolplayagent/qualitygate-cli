use super::*;
use anyhow::{Context, Result, bail};
use roxmltree::{Document, Node};

pub(super) fn parse(format: ReportFormat, text: &str) -> Result<Data> {
    if text.contains("<!DOCTYPE") {
        bail!("DTD is not allowed in tool reports");
    }
    let document = Document::parse(text)?;
    let root = document.root_element();
    let mut data = Data::default();
    let valid_root = match format {
        ReportFormat::Junit => ["testsuite", "testsuites"].contains(&root.tag_name().name()),
        ReportFormat::Checkstyle => root.has_tag_name("checkstyle"),
        ReportFormat::Spotbugs => root.has_tag_name("BugCollection"),
        ReportFormat::Pmd => root.has_tag_name("pmd"),
        ReportFormat::Cobertura => root.has_tag_name("coverage"),
        ReportFormat::Jacoco => root.has_tag_name("report"),
        _ => false,
    };
    if !valid_root {
        bail!("Unexpected XML root for {format:?}");
    }
    match format {
        ReportFormat::Junit => junit(root, &mut data)?,
        ReportFormat::Checkstyle | ReportFormat::Pmd => file_diagnostics(format, root, &mut data)?,
        ReportFormat::Spotbugs => spotbugs(root, &mut data)?,
        ReportFormat::Cobertura | ReportFormat::Jacoco => {
            super::coverage::xml(format, root, &mut data)?
        }
        _ => unreachable!(),
    }
    Ok(data)
}

fn junit(root: Node<'_, '_>, data: &mut Data) -> Result<()> {
    let mut tests = Tests::default();
    let cases: Vec<_> = root
        .descendants()
        .filter(|node| node.has_tag_name("testcase"))
        .collect();
    for case in &cases {
        if case.children().any(|node| node.has_tag_name("skipped")) {
            tests.skipped += 1;
            continue;
        }
        tests.executed += 1;
        let failures: Vec<_> = case
            .children()
            .filter(|node| node.has_tag_name("failure") || node.has_tag_name("error"))
            .collect();
        if failures.is_empty() {
            continue;
        }
        tests.failures += 1;
        data.issues.push(Issue {
            rule: "test-failure".into(),
            file: case.attribute("file").map(Into::into),
            line: optional_number(*case, "line")?,
            message: failures
                .iter()
                .map(|node| {
                    node.attribute("message")
                        .or_else(|| node.text())
                        .unwrap_or("Test failed")
                })
                .collect::<Vec<_>>()
                .join("; "),
            symbol: Some(format!(
                "{}#{}",
                case.attribute("classname").unwrap_or(""),
                case.attribute("name").unwrap_or("")
            )),
        });
    }
    if let Some(total) = optional_number(root, "tests")?
        && total != cases.len()
    {
        bail!("JUnit total does not match testcase records");
    }
    if root.descendants().any(|node| {
        node.has_tag_name("error")
            && node
                .parent()
                .is_some_and(|parent| !parent.has_tag_name("testcase"))
    }) {
        bail!("JUnit suite contains an execution error outside a test case");
    }
    data.tests = Some(tests);
    Ok(())
}

fn file_diagnostics(format: ReportFormat, root: Node<'_, '_>, data: &mut Data) -> Result<()> {
    let tag = if format == ReportFormat::Checkstyle {
        "error"
    } else {
        "violation"
    };
    for issue in root.descendants().filter(|node| node.has_tag_name(tag)) {
        let file = issue
            .ancestors()
            .find(|node| node.has_tag_name("file"))
            .and_then(|node| node.attribute("name"))
            .context("Static diagnostic has no file name")?;
        let rule = if format == ReportFormat::Checkstyle {
            issue.attribute("source")
        } else {
            issue.attribute("rule")
        }
        .unwrap_or(tag);
        let message = issue
            .attribute("message")
            .or_else(|| issue.text())
            .context("Diagnostic message missing")?;
        let line = optional_number(
            issue,
            if format == ReportFormat::Checkstyle {
                "line"
            } else {
                "beginline"
            },
        )?;
        data.issues.push(Issue {
            rule: rule.into(),
            file: Some(file.into()),
            line,
            message: message.trim().into(),
            symbol: None,
        });
    }
    if root.descendants().any(|node| {
        node.has_tag_name("error") && format == ReportFormat::Pmd
            || node.has_tag_name("configerror")
    }) {
        bail!("Static analyzer reported an execution/configuration error");
    }
    Ok(())
}

fn spotbugs(root: Node<'_, '_>, data: &mut Data) -> Result<()> {
    if root.descendants().any(|node| node.has_tag_name("Error")) {
        bail!("SpotBugs reported an analysis error");
    }
    for bug in root
        .descendants()
        .filter(|node| node.has_tag_name("BugInstance"))
    {
        let source = bug
            .descendants()
            .find(|node| {
                node.has_tag_name("SourceLine") && node.attribute("primary") == Some("true")
            })
            .or_else(|| {
                bug.descendants()
                    .find(|node| node.has_tag_name("SourceLine"))
            });
        data.issues.push(Issue {
            rule: bug
                .attribute("type")
                .context("SpotBugs bug type missing")?
                .into(),
            file: source
                .and_then(|node| {
                    node.attribute("sourcepath")
                        .or_else(|| node.attribute("sourcefile"))
                })
                .map(Into::into),
            line: source
                .map(|node| optional_number(node, "start"))
                .transpose()?
                .flatten(),
            message: bug
                .children()
                .find(|node| node.has_tag_name("LongMessage") || node.has_tag_name("ShortMessage"))
                .and_then(|node| node.text())
                .unwrap_or_else(|| bug.attribute("type").unwrap_or("Bug"))
                .into(),
            symbol: bug
                .descendants()
                .find(|node| node.has_tag_name("Method"))
                .and_then(|node| node.attribute("name"))
                .map(Into::into),
        });
    }
    Ok(())
}

pub(super) fn optional_number(node: Node<'_, '_>, name: &str) -> Result<Option<usize>> {
    node.attribute(name)
        .map(|value| {
            value
                .parse()
                .with_context(|| format!("Invalid XML numeric attribute {name}"))
        })
        .transpose()
}

pub(super) fn number(node: Node<'_, '_>, name: &str) -> Result<usize> {
    optional_number(node, name)?.with_context(|| format!("Missing XML numeric attribute {name}"))
}
