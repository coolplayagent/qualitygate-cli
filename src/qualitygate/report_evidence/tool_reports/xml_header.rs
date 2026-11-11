//! Skip only recognized inert report declarations; never resolve a DTD or entity.
use crate::config::ReportFormat;
use anyhow::{Context, Result, bail};
use std::borrow::Cow;

fn space(input: &str) -> &str {
    input.trim_start_matches([' ', '\t', '\r', '\n'])
}

fn word<'a>(input: &mut &'a str) -> Result<&'a str> {
    let separated = space(input);
    if separated.len() == input.len() {
        bail!("Report DOCTYPE tokens require whitespace");
    }
    let end = separated
        .find([' ', '\t', '\r', '\n', '>'])
        .context("Incomplete report DOCTYPE")?;
    let word = &separated[..end];
    *input = &separated[end..];
    Ok(word)
}

fn quoted<'a>(input: &mut &'a str) -> Result<&'a str> {
    let separated = space(input);
    if separated.len() == input.len() {
        bail!("Report DOCTYPE identifiers require whitespace");
    }
    let quote = separated
        .chars()
        .next()
        .context("Missing DOCTYPE identifier")?;
    if quote != '\'' && quote != '"' {
        bail!("DOCTYPE identifier must be quoted");
    }
    let tail = &separated[1..];
    let end = tail.find(quote).context("Unclosed DOCTYPE identifier")?;
    *input = &tail[end + 1..];
    Ok(&tail[..end])
}

fn recognized(format: ReportFormat, declaration: &str) -> Result<()> {
    let mut input = &declaration["<!DOCTYPE".len()..];
    let root = word(&mut input)?;
    let kind = word(&mut input)?;
    let first = quoted(&mut input)?;
    let public = (kind == "PUBLIC").then_some(first);
    let system = if public.is_some() {
        quoted(&mut input)?
    } else {
        first
    };
    if space(input) != ">" {
        bail!("Internal DTD subsets and extra declarations are not supported");
    }
    let allowed = match format {
        ReportFormat::Jacoco => {
            root == "report"
                && kind == "PUBLIC"
                && public == Some("-//JACOCO//DTD Report 1.1//EN")
                && system == "report.dtd"
        }
        ReportFormat::Cobertura => root == "coverage"
            && kind == "SYSTEM"
            && [
                "http://cobertura.sourceforge.net/xml/coverage-04.dtd",
                "https://cobertura.sourceforge.net/xml/coverage-04.dtd",
                "https://raw.githubusercontent.com/cobertura/web/master/htdocs/xml/coverage-04.dtd",
            ]
            .contains(&system),
        _ => false,
    };
    if !allowed {
        bail!("Unrecognized report DOCTYPE");
    }
    Ok(())
}

pub(super) fn normalize(format: ReportFormat, text: &str) -> Result<Cow<'_, str>> {
    let mut tail = text.strip_prefix('\u{feff}').unwrap_or(text);
    loop {
        tail = space(tail);
        let end = if tail.starts_with("<?") {
            Some("?>")
        } else if tail.starts_with("<!--") {
            Some("-->")
        } else {
            None
        };
        if let Some(end) = end {
            let offset = tail.find(end).context("Unclosed XML prolog token")?;
            tail = &tail[offset + end.len()..];
            continue;
        }
        if tail.starts_with("<!DOCTYPE") {
            let end = tail.find('>').context("Unclosed report DOCTYPE")? + 1;
            if end > 1024 {
                bail!("Report DOCTYPE exceeds 1 KiB");
            }
            recognized(format, &tail[..end])?;
            let start = text.len() - tail.len();
            let mut normalized = text.as_bytes().to_vec();
            for byte in &mut normalized[start..start + end] {
                if !b"\r\n".contains(byte) {
                    *byte = b' ';
                }
            }
            return Ok(Cow::Owned(String::from_utf8(normalized)?));
        }
        return Ok(Cow::Borrowed(text));
    }
}
