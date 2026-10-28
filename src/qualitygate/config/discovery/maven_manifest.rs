//! Advisory Maven manifest validation, separate from adapter report normalization.
//! Inventory acquisition owns the byte budget; discovery needs only a valid root.

use anyhow::{Context, Result, bail};
use xmlparser::{ElementEnd, Token, Tokenizer};

pub(super) fn validate(text: &str) -> Result<()> {
    let mut depth = 0_usize;
    // Preflight iteratively: the tree parser can exhaust portable thread stacks
    // before it reports a malformed or excessively nested manifest.
    for token in Tokenizer::from(text) {
        match token.context("Invalid Maven XML")? {
            Token::ElementStart { .. } => {
                depth += 1;
                if depth > 32 {
                    bail!("Maven manifest nesting exceeds 32 elements");
                }
            }
            Token::ElementEnd {
                end: ElementEnd::Close(..) | ElementEnd::Empty,
                ..
            } => {
                depth = depth
                    .checked_sub(1)
                    .context("Unbalanced Maven XML elements")?
            }
            _ => {}
        }
    }
    let document = roxmltree::Document::parse_with_options(
        text,
        roxmltree::ParsingOptions {
            nodes_limit: 100_000,
            ..Default::default()
        },
    )
    .context("Invalid Maven XML")?;
    if document.root_element().tag_name().name() != "project" {
        bail!("Maven manifest root must be project");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovery_depth_is_bounded_without_counting_literal_markup() {
        let document = |depth: usize| {
            format!(
                "<project>{}{}</project>",
                "<n>".repeat(depth - 1),
                "</n>".repeat(depth - 1)
            )
        };
        assert!(validate(&document(32)).is_ok());
        for depth in [33, 128, 2_000, 50_000] {
            let error = validate(&document(depth)).unwrap_err();
            assert!(error.to_string().contains("nesting exceeds"), "{error:#}");
        }
        let literal = "<n>".repeat(2_000);
        assert!(validate(&format!("<?xml version='1.0'?><project value='&lt;n&gt;'><!--{literal}--><?test {literal}?><![CDATA[{literal}]]><n/><n/></project>")).is_ok());
    }

    #[test]
    fn discovery_node_limit_includes_the_document_root() {
        let document = format!("<project>{}</project>", "<n/>".repeat(99_998));
        assert!(validate(&document).is_ok());
        let oversized = document.replace("</project>", "<n/></project>");
        let error = validate(&oversized).unwrap_err();
        assert!(matches!(
            error.downcast_ref(),
            Some(roxmltree::Error::NodesLimitReached)
        ));
    }

    #[test]
    fn discovery_retains_well_formedness_namespace_and_root_validation() {
        for text in [
            "",
            "<project>",
            "<project><n></project>",
            "<project/><project/>",
            "<project value='unterminated>",
            "<project a='1' a='2'/>",
            "<project>&missing;</project>",
            "<project><missing:n/></project>",
            "<!DOCTYPE project [<!ENTITY x 'text'>]><project>&x;</project>",
        ] {
            let error = validate(text).unwrap_err();
            assert!(
                format!("{error:#}").contains("Invalid Maven XML"),
                "{text}: {error:#}"
            );
        }
        assert!(
            validate("<other/>")
                .unwrap_err()
                .to_string()
                .contains("root must be project")
        );
        for text in [
            "<project/>",
            "<project xmlns='urn:maven'/>",
            "<m:project xmlns:m='urn:maven'/>",
        ] {
            assert!(validate(text).is_ok(), "{text}");
        }
    }
}
