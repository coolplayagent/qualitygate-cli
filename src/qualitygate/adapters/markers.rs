//! Explicit declaration bindings. Presence is not a claim of actual provenance.

use super::syntax::{Entity, Structure};
use crate::config::Marker;
use anyhow::{Result, bail};
use std::collections::BTreeMap;

pub(super) fn bound_declaration(
    marker: &Marker,
    entity: &Entity,
    view: &Structure,
    path: &str,
    baseline: bool,
    git: Option<&super::git_trailers::GitFacts>,
) -> Result<Option<String>> {
    if marker.kind == "git_trailer" {
        return git
            .ok_or_else(|| anyhow::anyhow!("Verified commit-to-entity facts are unavailable"))?
            .declaration(marker, path, entity, baseline);
    }
    from_source(marker, entity, view)
}

pub(super) fn from_source(
    marker: &Marker,
    entity: &Entity,
    view: &Structure,
) -> Result<Option<String>> {
    match marker.kind.as_str() {
        "annotation" => Ok(entity.annotations.get(&marker.name).cloned()),
        "comment" => {
            let mut end = entity.range.start_line;
            let mut parts = Vec::new();
            let preceding = view
                .comments
                .partition_point(|comment| comment.range.start_line < end);
            for comment in view.comments[..preceding]
                .iter()
                .rev()
                .filter(|comment| comment.range.end_line < entity.range.start_line)
            {
                let next_line = view
                    .nonblank_lines
                    .partition_point(|line| *line <= comment.range.end_line);
                if view
                    .nonblank_lines
                    .get(next_line)
                    .is_some_and(|line| *line < end)
                {
                    break;
                }
                parts.push(comment.text.as_str());
                end = comment.range.start_line;
            }
            parts.reverse();
            let content = parts.join("\n");
            let found = content.lines().any(|line| {
                line.trim_start_matches(|character: char| {
                    character.is_whitespace() || ['/', '*', '#'].contains(&character)
                })
                .strip_prefix(&marker.name)
                .is_some_and(|rest| {
                    rest.is_empty()
                        || rest.starts_with(|character: char| {
                            character.is_whitespace() || [':', '('].contains(&character)
                        })
                })
            });
            Ok(found.then_some(content))
        }
        "git_trailer" => {
            // A range-level trailer cannot identify the commit introducing an entity.
            // Association must be supplied by the Git provenance adapter.
            bail!("Commit-to-entity provenance is required for git_trailer binding")
        }
        other => bail!("Unsupported declaration binding: {other}"),
    }
}

/// Parse assignments outside quoted strings, preventing a description containing
/// `author='somebody'` from masquerading as an author field.
pub(super) fn fields(text: &str) -> BTreeMap<String, String> {
    let bytes = text.as_bytes();
    let mut values = BTreeMap::new();
    let mut offset = 0;
    while offset < bytes.len() {
        if matches!(bytes[offset], b'\'' | b'"') {
            let (_, next) = quoted(bytes, offset);
            offset = next;
            continue;
        }
        if !bytes[offset].is_ascii_alphabetic() && bytes[offset] != b'_' {
            offset += 1;
            continue;
        }
        let start = offset;
        while offset < bytes.len()
            && (bytes[offset].is_ascii_alphanumeric()
                || matches!(bytes[offset], b'_' | b'-' | b'.'))
        {
            offset += 1;
        }
        let name_end = offset;
        while offset < bytes.len() && bytes[offset].is_ascii_whitespace() {
            offset += 1;
        }
        if offset >= bytes.len() || !matches!(bytes[offset], b':' | b'=') {
            continue;
        }
        offset += 1;
        while offset < bytes.len() && bytes[offset].is_ascii_whitespace() {
            offset += 1;
        }
        if offset == bytes.len() {
            break;
        }
        let value = if matches!(bytes[offset], b'\'' | b'"') {
            let (value, next) = quoted(bytes, offset);
            offset = next;
            value
        } else {
            let start = offset;
            while offset < bytes.len()
                && !bytes[offset].is_ascii_whitespace()
                && !b",;)}/".contains(&bytes[offset])
            {
                offset += 1;
            }
            String::from_utf8_lossy(&bytes[start..offset]).into_owned()
        };
        let key = String::from_utf8_lossy(&bytes[start..name_end]).into_owned();
        // Repeated fields are ambiguous evidence, including one empty occurrence.
        values
            .entry(key)
            .and_modify(|value: &mut String| value.clear())
            .or_insert(value);
    }
    values
}

fn quoted(bytes: &[u8], start: usize) -> (String, usize) {
    let mut offset = start + 1;
    while offset < bytes.len() {
        if bytes[offset] == b'\\' {
            offset += 2;
        } else if bytes[offset] == bytes[start] {
            return (
                String::from_utf8_lossy(&bytes[start + 1..offset])
                    .trim()
                    .into(),
                offset + 1,
            );
        } else {
            offset += 1;
        }
    }
    (String::new(), bytes.len())
}

#[cfg(test)]
#[path = "markers_tests.rs"]
mod tests;
