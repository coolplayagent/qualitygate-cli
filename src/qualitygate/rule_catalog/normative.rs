//! Exact CommonMark ATX section selection shared by authoring and snapshot checks.

use anyhow::{Result, bail};

pub fn section<'a>(text: &'a str, title: &str) -> Result<&'a str> {
    let mut headings = Vec::new();
    let mut depth = 0usize;
    for (event, range) in pulldown_cmark::Parser::new(text).into_offset_iter() {
        match event {
            pulldown_cmark::Event::Start(tag) => {
                if depth == 0
                    && let pulldown_cmark::Tag::Heading { level, .. } = tag
                {
                    let line = text[range.start..].lines().next().unwrap_or_default();
                    let hashes = line
                        .chars()
                        .take_while(|character| *character == '#')
                        .count();
                    if (1..=6).contains(&hashes) && line[hashes..].starts_with(' ') {
                        headings.push((level as usize, line[hashes..].trim(), range.start));
                    }
                }
                depth += 1;
            }
            pulldown_cmark::Event::End(_) => depth -= 1,
            _ => {}
        }
    }
    let matching: Vec<_> = headings
        .iter()
        .enumerate()
        .filter(|(_, (_, name, _))| *name == title)
        .collect();
    if matching.len() != 1 {
        bail!(
            "Normative section must exist exactly once: {title} (found {})",
            matching.len()
        );
    }
    let (index, (level, _, start)) = matching[0];
    let end = headings[index + 1..]
        .iter()
        .find(|(next_level, _, _)| next_level <= level)
        .map_or(text.len(), |(_, _, offset)| *offset);
    Ok(&text[*start..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_sections_keep_nested_bytes_and_reject_ambiguous_or_fake_headings() {
        let text = "# Title\r\n## Rules\r\nText.\r\n### Detail\r\nMore.\r\n## Other\r\nEnd.\r\n";
        assert_eq!(
            section(text, "Rules").unwrap(),
            "## Rules\r\nText.\r\n### Detail\r\nMore.\r\n"
        );
        assert_eq!(section(text, "Other").unwrap(), "## Other\r\nEnd.\r\n");
        assert!(section("## Rules\nA\n## Rules\nB\n", "Rules").is_err());
        for text in [
            "```md\n## Rules\n```\n",
            "> ## Rules\n",
            "<div>\n## Rules\n</div>\n",
            "Rules\n-----\n",
            "##Rules\n",
        ] {
            assert!(section(text, "Rules").is_err(), "accepted {text:?}");
        }
        // CommonMark permits up to three spaces; the existing selector binds
        // from its parser's ATX start offset, not preceding indentation.
        assert_eq!(section("  ## Rules\n", "Rules").unwrap(), "## Rules\n");
        assert_eq!(
            section("## **Rules**\nBody.\n", "**Rules**").unwrap(),
            "## **Rules**\nBody.\n"
        );
    }
}
