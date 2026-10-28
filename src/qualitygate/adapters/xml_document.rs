//! Bounded XML tree parsing shared by report and project adapters.
//! Callers retain their input byte budgets and format-specific header handling.

use anyhow::{Context, Result, bail};
use xmlparser::{ElementEnd, Token, Tokenizer};

// The recursive parser's debug stack frames vary by platform; 128 levels
// overflow on macOS. Leave headroom for smaller thread stacks.
pub(super) const MAX_DEPTH: usize = 32;

pub(super) fn parse(text: &str) -> Result<roxmltree::Document<'_>> {
    validate(text)?;
    Ok(roxmltree::Document::parse_with_options(
        text,
        roxmltree::ParsingOptions {
            nodes_limit: 100_000,
            ..Default::default()
        },
    )?)
}

fn validate(text: &str) -> Result<()> {
    let mut depth = 0_usize;
    let mut element_attributes = 0_usize;
    let mut document_attributes = 0_usize;
    let mut namespaces = crate::domain::xml_budget::NamespaceBudget::default();
    let mut declarations = 0_usize;
    let mut lookups = 1_usize;
    for token in Tokenizer::from(text) {
        match token? {
            Token::ElementStart { .. } => {
                element_attributes = 0;
                declarations = 0;
                lookups = 1;
                depth += 1;
                if depth > MAX_DEPTH {
                    bail!("XML report nesting exceeds {MAX_DEPTH} elements");
                }
            }
            Token::Attribute { prefix, local, .. } => {
                // Namespace declarations are attributes too. The tree parser's
                // duplicate checks can otherwise perform quadratic work before
                // its node limit applies to an attribute-heavy element.
                element_attributes += 1;
                document_attributes += 1;
                if element_attributes > 1_024 {
                    bail!("XML element exceeds 1024 attributes");
                }
                if document_attributes > 100_000 {
                    bail!("XML document exceeds 100000 attributes");
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
                        .context("Unbalanced XML report elements")?;
                    namespaces.close().map_err(anyhow::Error::msg)?;
                }
            }
            _ => {}
        }
    }
    // Leave incomplete elements and name matching to the tree parser so its
    // existing malformed-document diagnostics remain authoritative.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xml_depth_counts_elements_without_counting_attributes_or_literal_markup() {
        let nested = |depth| format!("{}{}", "<a>".repeat(depth), "</a>".repeat(depth));
        assert!(validate(&nested(MAX_DEPTH)).is_ok());
        for depth in [MAX_DEPTH + 1, 128, 2_000, 50_000] {
            assert!(
                validate(&nested(depth))
                    .unwrap_err()
                    .to_string()
                    .contains("nesting exceeds")
            );
        }
        let literal = "<a>".repeat(2_000);
        let input = format!(
            "<?xml version='1.0'?><a value='&lt;a&gt;'><!--{literal}--><?test {literal}?><![CDATA[{literal}]]><b/><b/></a>"
        );
        assert!(validate(&input).is_ok());
        for input in ["<a>", "<a></a></a>", "<a value='unterminated>"] {
            assert!(parse(input).is_err(), "{input}");
        }
    }

    #[test]
    fn xml_node_limit_counts_the_document_root_and_rejects_wide_inputs() {
        let document = format!("<root>{}</root>", "<n/>".repeat(99_998));
        assert_eq!(parse(&document).unwrap().descendants().count(), 100_000);
        let oversized = document.replace("</root>", "<n/></root>");
        assert!(matches!(
            parse(&oversized).unwrap_err().downcast_ref(),
            Some(roxmltree::Error::NodesLimitReached)
        ));
    }

    #[test]
    fn xml_attribute_limits_count_namespaces_and_reset_for_each_element() {
        let attributes = |count: usize| {
            (0..count)
                .map(|index| format!("a{index}='v' "))
                .collect::<String>()
        };
        let exact = attributes(1_024);
        assert!(
            parse(&format!(
                "<root {exact}><child {exact}/><child {exact}/></root>"
            ))
            .is_ok()
        );
        for extra in ["extra='v'", "xmlns='urn:test'", "xmlns:p='urn:test'"] {
            let error = parse(&format!("<root {exact}{extra}/>")).unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains("element exceeds 1024 attributes")
            );
        }
        let namespaces = (0..1_024)
            .map(|index| format!("xmlns:p{index}='urn:{index}' "))
            .collect::<String>();
        assert!(parse(&format!("<root {namespaces}/>")).is_ok());
        assert!(parse(&format!("<root {namespaces}xmlns:extra='urn:extra'/>")).is_err());
        let literal = "a='v' ".repeat(2_000);
        assert!(parse(&format!("<root value=\"{literal}\"><!--{literal}--><?test {literal}?><![CDATA[{literal}]]></root>")).is_ok());
        assert!(parse("<root a='1' a='2'/>").is_err());
    }

    #[test]
    fn xml_attribute_budget_spans_the_document_independently_of_nodes() {
        let exact = format!("<root>{}</root>", "<n a='1' b='2'/>".repeat(50_000));
        assert!(parse(&exact).is_ok());
        let oversized = exact.replace("</root>", "<n a='1'/></root>");
        let error = parse(&oversized).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("document exceeds 100000 attributes")
        );
    }

    #[test]
    fn xml_namespace_work_bounds_inherited_scopes_and_qualified_names() {
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
            let document = |count| format!("<root {declarations}>{}</root>", child.repeat(count));
            assert!(parse(&document(9)).is_ok());
            let error = parse(&document(10)).unwrap_err();
            assert!(error.to_string().contains("namespace resolution exceeds"));
        }
        assert!(
            parse(&format!(
                "<root xmlns:p='urn:p'>{}</root>",
                "<p:n xmlns:p='urn:p'/>".repeat(10_000)
            ))
            .is_ok()
        );
    }
}
