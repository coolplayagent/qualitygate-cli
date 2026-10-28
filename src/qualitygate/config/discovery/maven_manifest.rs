//! Advisory Maven manifest validation, separate from adapter report normalization.
//! Inventory acquisition owns the byte budget; discovery needs only a valid root.

use anyhow::{Context, Result, bail};
use xmlparser::{ElementEnd, Token, Tokenizer};

pub(super) fn validate(text: &str) -> Result<()> {
    let mut depth = 0_usize;
    let mut element_attributes = 0_usize;
    let mut document_attributes = 0_usize;
    let mut namespaces = crate::domain::xml_budget::NamespaceBudget::default();
    let mut declarations = 0_usize;
    let mut lookups = 1_usize;
    // Preflight iteratively: the tree parser can exhaust portable thread stacks
    // before it reports a malformed or excessively nested manifest.
    for token in Tokenizer::from(text) {
        match token.context("Invalid Maven XML")? {
            Token::ElementStart { .. } => {
                element_attributes = 0;
                declarations = 0;
                lookups = 1;
                depth += 1;
                if depth > 32 {
                    bail!("Maven manifest nesting exceeds 32 elements");
                }
            }
            Token::Attribute { prefix, local, .. } => {
                // Count namespace declarations as well as ordinary attributes
                // before the tree parser performs duplicate-name comparisons.
                element_attributes += 1;
                document_attributes += 1;
                if element_attributes > 1_024 {
                    bail!("Maven manifest element exceeds 1024 attributes");
                }
                if document_attributes > 100_000 {
                    bail!("Maven manifest document exceeds 100000 attributes");
                }
                if prefix.as_str() == "xmlns" || (prefix.is_empty() && local.as_str() == "xmlns") {
                    declarations += 1;
                } else if !prefix.is_empty() {
                    lookups += 1;
                }
            }
            Token::ElementEnd { end, .. } => {
                if matches!(end, ElementEnd::Open | ElementEnd::Empty) {
                    namespaces
                        .open(declarations, lookups)
                        .map_err(anyhow::Error::msg)?;
                }
                if matches!(end, ElementEnd::Close(..) | ElementEnd::Empty) {
                    depth = depth
                        .checked_sub(1)
                        .context("Unbalanced Maven XML elements")?;
                    namespaces.close().map_err(anyhow::Error::msg)?;
                }
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

    #[test]
    fn discovery_attribute_limit_includes_namespaces_and_resets_per_element() {
        let attributes = (0..1_024)
            .map(|index| format!("a{index}='v' "))
            .collect::<String>();
        assert!(
            validate(&format!(
                "<project {attributes}><n {attributes}/><n {attributes}/></project>"
            ))
            .is_ok()
        );
        for extra in ["extra='v'", "xmlns='urn:test'", "xmlns:p='urn:test'"] {
            let error = validate(&format!("<project {attributes}{extra}/>")).unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains("element exceeds 1024 attributes")
            );
        }
        let namespaces = (0..1_024)
            .map(|index| format!("xmlns:p{index}='urn:{index}' "))
            .collect::<String>();
        assert!(validate(&format!("<project {namespaces}/>")).is_ok());
        assert!(validate(&format!("<project {namespaces}xmlns:extra='urn:extra'/>")).is_err());
        let literal = "a='v' ".repeat(2_000);
        assert!(validate(&format!("<project value=\"{literal}\"><!--{literal}--><?test {literal}?><![CDATA[{literal}]]></project>")).is_ok());
    }

    #[test]
    fn discovery_attribute_budget_covers_all_elements() {
        let exact = format!("<project>{}</project>", "<n a='1' b='2'/>".repeat(50_000));
        assert!(validate(&exact).is_ok());
        let oversized = exact.replace("</project>", "<n a='1'/></project>");
        let error = validate(&oversized).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("document exceeds 100000 attributes")
        );
    }

    #[test]
    fn discovery_namespace_work_bounds_scope_expansion_and_name_resolution() {
        let declarations = (0..1_000)
            .map(|i| format!("xmlns:p{i}='urn:{i}' "))
            .collect::<String>();
        let attributes = (0..1_000)
            .map(|i| format!("p999:a{i}='v' "))
            .collect::<String>();
        for child in [
            "<n xmlns:q='urn:q'/>".to_owned(),
            format!("<n {attributes}/>"),
        ] {
            let document =
                |count| format!("<project {declarations}>{}</project>", child.repeat(count));
            assert!(validate(&document(9)).is_ok());
            let error = validate(&document(10)).unwrap_err();
            assert!(error.to_string().contains("namespace resolution exceeds"));
        }
        assert!(
            validate(&format!(
                "<project xmlns:p='urn:p'>{}</project>",
                "<p:n xmlns:p='urn:p'/>".repeat(10_000)
            ))
            .is_ok()
        );
    }
}
