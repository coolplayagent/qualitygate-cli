//! Strict JUnit profile for paired effectiveness; ordinary JUnit is unchanged.

use crate::{
    config::ReportFormat,
    domain::test_effectiveness::{TestCase, TestOutcome},
    paths,
    snapshot::Snapshot,
};
use anyhow::{Context, Result, bail};
use std::path::Path;

pub fn parse(bytes: &[u8], snapshot: &Snapshot, workspace: &Path) -> Result<Vec<TestCase>> {
    if bytes.len() > crate::snapshot::MAX_FILE_BYTES {
        bail!("Test report exceeds size budget");
    }
    let text = super::xml_header::normalize(ReportFormat::Junit, std::str::from_utf8(bytes)?)?;
    let doc = roxmltree::Document::parse_with_options(
        &text,
        roxmltree::ParsingOptions {
            nodes_limit: 100_000,
            ..Default::default()
        },
    )?;
    let root = doc.root_element();
    if !["testsuite", "testsuites"].contains(&root.tag_name().name()) {
        bail!("Unexpected JUnit root");
    }
    let mut cases = Vec::new();
    for node in root.descendants().filter(|n| n.has_tag_name("testcase")) {
        if cases.len() >= 50_000 {
            bail!("Test report exceeds 50000 cases");
        }
        let raw = node
            .attribute("file")
            .context("JUnit testcase needs an explicit file")?;
        let path = Path::new(raw);
        let file = if path.is_absolute() {
            paths::from_native(
                path.strip_prefix(workspace)
                    .context("Test file is outside the execution workspace")?,
            )?
        } else {
            paths::relative(path)?
        };
        if file.is_empty() || !snapshot.files.contains_key(&file) {
            bail!("Test file does not map to a captured input: {raw}");
        }
        let name = node
            .attribute("name")
            .filter(|name| !name.trim().is_empty())
            .context("JUnit testcase needs a name")?;
        let statuses: Vec<_> = node
            .children()
            .filter(|n| {
                n.is_element() && ["failure", "error", "skipped"].contains(&n.tag_name().name())
            })
            .collect();
        if statuses
            .iter()
            .any(|n| n.tag_name().name() != statuses[0].tag_name().name())
            || (statuses.len() > 1 && !statuses[0].has_tag_name("failure"))
        {
            bail!("JUnit testcase has contradictory outcomes");
        }
        let outcome = match statuses.first().map(|n| n.tag_name().name()) {
            Some("failure") => TestOutcome::Failure {
                types: statuses
                    .iter()
                    .map(|n| n.attribute("type").unwrap_or("").into())
                    .collect(),
            },
            Some("error") => TestOutcome::Error,
            Some("skipped") => TestOutcome::Skipped,
            _ => TestOutcome::Passed,
        };
        if !node.parent().is_some_and(|n| n.has_tag_name("testsuite")) {
            bail!("JUnit testcase must be a direct child of a testsuite");
        }
        cases.push(TestCase {
            file,
            classname: node.attribute("classname").unwrap_or("").into(),
            name: name.into(),
            outcome,
        });
    }
    validate_counters(root)?;
    Ok(cases)
}

fn validate_counters(root: roxmltree::Node<'_, '_>) -> Result<()> {
    // Aggregate once bottom-up; nested suites must not rescan every descendant.
    let nodes: Vec<_> = root.descendants().filter(|n| n.is_element()).collect();
    let mut totals = std::collections::BTreeMap::<u32, [usize; 4]>::new();
    for node in nodes.into_iter().rev() {
        let tag = node.tag_name().name();
        if ["error", "failure", "skipped"].contains(&tag)
            && !node.parent().is_some_and(|n| n.has_tag_name("testcase"))
        {
            bail!("JUnit outcome outside a testcase");
        }
        let counts = if tag == "testcase" {
            [
                1,
                usize::from(node.children().any(|n| n.has_tag_name("failure"))),
                usize::from(node.children().any(|n| n.has_tag_name("error"))),
                usize::from(node.children().any(|n| n.has_tag_name("skipped"))),
            ]
        } else if ["testsuite", "testsuites"].contains(&tag) {
            let counts = totals.remove(&node.id().get()).unwrap_or_default();
            for (attribute, count) in ["tests", "failures", "errors", "skipped"]
                .into_iter()
                .zip(counts)
            {
                if let Some(value) = super::xml::optional_number(node, attribute)?
                    && value != count
                {
                    bail!("JUnit {attribute} counter contradicts testcase records");
                }
            }
            counts
        } else {
            continue;
        };
        if node == root {
            continue;
        }
        let parent = node.parent().context("JUnit record has no parent")?;
        if !["testsuite", "testsuites"].contains(&parent.tag_name().name()) {
            bail!("JUnit suite or testcase has an unsupported parent");
        }
        let total = totals.entry(parent.id().get()).or_default();
        for (sum, count) in total.iter_mut().zip(counts) {
            *sum += count;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::{File, Identity};

    fn snapshot() -> Snapshot {
        Snapshot {
            root: "/fixture".into(),
            identity: Identity {
                mode: "worktree".into(),
                base: "base".into(),
                head: "head".into(),
                content_digest: "digest".into(),
                merge_request: None,
            },
            files: [(
                "tests/a.rs".into(),
                File {
                    bytes: b"test\n".to_vec(),
                    executable: false,
                },
            )]
            .into(),
            base_files: Default::default(),
            changes: Default::default(),
            path_filter: None,
            commits: vec![],
        }
    }
    #[test]
    fn junit_case_evidence_rejects_ambiguous_shapes_without_changing_ordinary_junit() {
        let input = snapshot();
        let workspace = Path::new("/fixture");
        let good = "<testsuite tests='1' failures='1' errors='0' skipped='0'><testcase file='tests/a.rs' classname='A' name='checks'><failure type='AssertionError'/></testcase></testsuite>";
        assert!(matches!(
            parse(good.as_bytes(), &input, workspace).unwrap()[0].outcome,
            TestOutcome::Failure { .. }
        ));
        for bad in [
            "<other/>",
            "<testsuite><error/></testsuite>",
            "<testsuite><testcase name='a'/></testsuite>",
            "<testsuite><testcase file='absent' name='a'/></testsuite>",
            "<testsuite><testcase file='../outside' name='a'/></testsuite>",
            "<testsuite><testcase file='tests/a.rs'/></testsuite>",
            "<testsuite><testcase file='tests/a.rs' name='a'><failure/><skipped/></testcase></testsuite>",
            "<testsuite><testcase file='tests/a.rs' name='a'><skipped/><skipped/></testcase></testsuite>",
            "<testsuite><testcase file='tests/a.rs' name='a'><testcase file='tests/a.rs' name='b'/></testcase></testsuite>",
            "<testsuite tests='1'><testsuite tests='2'><testcase file='tests/a.rs' name='a'/></testsuite></testsuite>",
            "<testsuite errors='no'/>",
        ] {
            assert!(parse(bad.as_bytes(), &input, workspace).is_err(), "{bad}");
        }
        assert!(
            parse(
                &vec![b'x'; crate::snapshot::MAX_FILE_BYTES + 1],
                &input,
                workspace
            )
            .is_err()
        );
        assert!(parse(b"\xff", &input, workspace).is_err());
        let nested = format!("<testsuites tests='1' failures='1'>{good}</testsuites>");
        assert_eq!(
            parse(nested.as_bytes(), &input, workspace).unwrap().len(),
            1
        );
        let ordinary = b"<testsuite><testcase name='ordinary'><error message='failed'/></testcase></testsuite>";
        assert_eq!(
            super::super::parse(ReportFormat::Junit, ordinary)
                .unwrap()
                .tests
                .unwrap()
                .failures,
            1
        );
        assert!(parse(ordinary, &input, workspace).is_err());
        // Empty and opaque failure types remain visible for the pure domain to reject.
        let missing_type = good.replace(" type='AssertionError'", "");
        assert!(
            matches!(&parse(missing_type.as_bytes(), &input, workspace).unwrap()[0].outcome,
            TestOutcome::Failure { types } if types == &vec![String::new()])
        );
    }
}
