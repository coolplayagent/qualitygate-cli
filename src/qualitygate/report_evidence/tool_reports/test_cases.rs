//! Paired JUnit evidence adds captured-file and test-identity requirements.

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
    let doc = crate::adapters::xml_document::parse(&text)?;
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
        let statuses = super::junit_validation::outcomes(node)?;
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
        cases.push(TestCase {
            file,
            classname: node.attribute("classname").unwrap_or("").into(),
            name: name.into(),
            outcome,
        });
    }
    super::junit_validation::validate(root)?;
    Ok(cases)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::{File, Identity};

    fn snapshot() -> Snapshot {
        Snapshot {
            scope_evidence: Default::default(),
            root: "/fixture".into(),
            identity: Identity {
                verification_digest: None,
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
    fn junit_case_evidence_requires_metadata_without_imposing_it_on_ordinary_junit() {
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
        let deep = format!(
            "{}<testcase file='tests/a.rs' name='a'/>{}",
            "<testsuite>".repeat(2_000),
            "</testsuite>".repeat(2_000)
        );
        assert!(
            parse(deep.as_bytes(), &input, workspace)
                .unwrap_err()
                .to_string()
                .contains("nesting exceeds")
        );
        let attributes = (0..1_025).map(|i| format!("a{i}='v' ")).collect::<String>();
        let declarations = (0..1_000)
            .map(|i| format!("xmlns:p{i}='urn:{i}' "))
            .collect::<String>();
        for (document, reason) in [
            (
                format!(
                    "<testsuite {declarations}>{}{good}</testsuite>",
                    "<n xmlns:q='urn:q'/>".repeat(10)
                ),
                "namespace resolution exceeds",
            ),
            (
                format!("<testsuite {attributes}>{good}</testsuite>"),
                "element exceeds 1024 attributes",
            ),
            (
                format!(
                    "<testsuite>{}{good}</testsuite>",
                    "<n a='1' b='2'/>".repeat(50_001)
                ),
                "document exceeds 100000 attributes",
            ),
        ] {
            assert!(
                parse(document.as_bytes(), &input, workspace)
                    .unwrap_err()
                    .to_string()
                    .contains(reason)
            );
        }
        // Empty and opaque failure types remain visible for the pure domain to reject.
        let missing_type = good.replace(" type='AssertionError'", "");
        assert!(
            matches!(&parse(missing_type.as_bytes(), &input, workspace).unwrap()[0].outcome,
            TestOutcome::Failure { types } if types == &vec![String::new()])
        );
    }
}
