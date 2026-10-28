//! Iterative depth preflight before the recursive XML tree parser.

use anyhow::{Context, Result, bail};
use xmlparser::{ElementEnd, Token, Tokenizer};

pub(super) const MAX_DEPTH: usize = 128;

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
    for token in Tokenizer::from(text) {
        match token? {
            Token::ElementStart { .. } => {
                depth += 1;
                if depth > MAX_DEPTH {
                    bail!("XML report nesting exceeds {MAX_DEPTH} elements");
                }
            }
            Token::ElementEnd {
                end: ElementEnd::Close(..) | ElementEnd::Empty,
                ..
            } => {
                depth = depth
                    .checked_sub(1)
                    .context("Unbalanced XML report elements")?
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
        for depth in [MAX_DEPTH + 1, 2_000, 50_000] {
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
}
