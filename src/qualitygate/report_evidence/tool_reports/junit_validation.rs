//! Structural and counter checks shared by ordinary and paired JUnit evidence.

use anyhow::{Context, Result, bail};
use roxmltree::Node;
use std::collections::BTreeMap;

pub(super) fn outcomes<'a, 'input>(case: Node<'a, 'input>) -> Result<Vec<Node<'a, 'input>>> {
    let statuses: Vec<_> = case
        .children()
        .filter(|node| {
            node.is_element() && ["failure", "error", "skipped"].contains(&node.tag_name().name())
        })
        .collect();
    if let Some(first) = statuses.first()
        && (statuses
            .iter()
            .any(|node| node.tag_name().name() != first.tag_name().name())
            || (statuses.len() > 1 && !first.has_tag_name("failure")))
    {
        bail!("JUnit testcase has contradictory outcomes");
    }
    Ok(statuses)
}

pub(super) fn validate(root: Node<'_, '_>) -> Result<()> {
    // Both callers bound XML nodes. Aggregate each record once bottom-up so
    // deeply nested suites cannot trigger repeated descendant scans.
    let nodes: Vec<_> = root
        .descendants()
        .filter(|node| node.is_element())
        .collect();
    let mut totals = BTreeMap::<u32, [usize; 4]>::new();
    for node in nodes.into_iter().rev() {
        let tag = node.tag_name().name();
        if ["error", "failure", "skipped"].contains(&tag)
            && !node
                .parent()
                .is_some_and(|parent| parent.has_tag_name("testcase"))
        {
            bail!("JUnit outcome outside a testcase");
        }
        let counts = if tag == "testcase" {
            if !node
                .parent()
                .is_some_and(|parent| parent.has_tag_name("testsuite"))
            {
                bail!("JUnit testcase must be a direct child of a testsuite");
            }
            let statuses = outcomes(node)?;
            let outcome = statuses.first().map(|status| status.tag_name().name());
            [
                1,
                usize::from(outcome == Some("failure")),
                usize::from(outcome == Some("error")),
                usize::from(outcome == Some("skipped")),
            ]
        } else if ["testsuite", "testsuites"].contains(&tag) {
            let counts = totals.remove(&node.id().get()).unwrap_or_default();
            for (attribute, count) in ["tests", "failures", "errors", "skipped"]
                .into_iter()
                .zip(counts)
            {
                let value = node
                    .attribute(attribute)
                    .map(|value| {
                        value
                            .parse::<usize>()
                            .with_context(|| format!("Invalid XML numeric attribute {attribute}"))
                    })
                    .transpose()?;
                if let Some(value) = value
                    && value != count
                {
                    bail!(
                        "JUnit {attribute} counter contradicts testcase records (declared {value}, observed {count})"
                    );
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
