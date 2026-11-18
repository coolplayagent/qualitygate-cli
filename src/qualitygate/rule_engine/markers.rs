//! Explicit declaration bindings. Presence is not a claim of actual provenance.

use crate::adapters::syntax::Entity;
use crate::adapters::syntax::Structure;
use crate::config::Marker;
use anyhow::{Result, bail};

pub(crate) fn bound_declaration(
    marker: &Marker,
    entity: &Entity,
    view: &Structure,
    path: &str,
    baseline: bool,
    git: Option<&crate::adapters::git_trailers::GitFacts>,
) -> Result<Option<String>> {
    if marker.kind == "git_trailer" {
        return git
            .ok_or_else(|| anyhow::anyhow!("Verified commit-to-entity facts are unavailable"))?
            .declaration(marker, path, entity, baseline);
    }
    from_source(marker, entity, view)
}

pub(crate) fn from_source(
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

#[cfg(test)]
#[path = "markers_tests.rs"]
mod tests;
