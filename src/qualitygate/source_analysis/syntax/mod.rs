//! Framework-aware syntax facts from bounded Tree-sitter parsing.

mod cache;
pub(crate) mod review;
mod tests_in_source;
pub use cache::Cache;
pub(crate) use cache::parse_cached;

use crate::domain::Range;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    ops::ControlFlow,
    time::{Duration, Instant},
};
use tree_sitter::{Language, Node, ParseOptions, Parser};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub name: String,
    pub symbol: String,
    pub range: Range,
    pub byte_range: std::ops::Range<usize>,
    pub annotations: BTreeMap<String, String>,
    pub body_digest: String,
    pub shape_digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment {
    pub text: String,
    pub range: Range,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Import {
    pub text: String,
    pub range: Range,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Structure {
    pub language: String,
    pub tests: Vec<Entity>,
    pub comments: Vec<Comment>,
    pub imports: Vec<Import>,
    /// One-based nonblank lines support declaration binding without rescanning
    /// the full source for every entity. This is invocation-local derived data.
    #[serde(skip)]
    pub(crate) nonblank_lines: Vec<usize>,
    /// Byte ranges into the corresponding Rust import texts, in import order.
    #[serde(skip)]
    rust_use_arguments: Vec<std::ops::Range<usize>>,
}

impl Structure {
    pub(crate) fn declared_import_text(&self, index: usize) -> Result<&str> {
        let import = self
            .imports
            .get(index)
            .context("Import syntax is unavailable")?;
        if self.language != "rust" {
            return Ok(&import.text);
        }
        self.rust_use_arguments
            .get(index)
            .and_then(|range| import.text.get(range.clone()))
            .context("Rust use argument syntax is unavailable")
    }
}

pub fn language(path: &str) -> Option<&'static str> {
    crate::domain::language::for_path(path).map(|language| language.name)
}

pub fn parse(path: &str, bytes: &[u8]) -> Result<Option<Structure>> {
    parse_before(path, bytes, Instant::now() + Duration::from_secs(2))
}

/// Parsing and fact extraction cannot outlive the caller's acquisition deadline.
pub(crate) fn parse_before(
    path: &str,
    bytes: &[u8],
    deadline: Instant,
) -> Result<Option<Structure>> {
    let deadline = deadline.min(Instant::now() + Duration::from_secs(2));
    let check_deadline = || -> Result<()> {
        anyhow::ensure!(
            Instant::now() < deadline,
            "Syntax parsing exceeded its budget: {path}"
        );
        Ok(())
    };
    check_deadline()?;
    let Some(language) = language(path) else {
        return Ok(None);
    };
    std::str::from_utf8(bytes).with_context(|| format!("Source file is not UTF-8: {path}"))?;
    let grammar: Language = match language {
        "java" => tree_sitter_java::LANGUAGE.into(),
        "python" => tree_sitter_python::LANGUAGE.into(),
        "typescript" if path.ends_with("tsx") || path.ends_with("jsx") => {
            tree_sitter_typescript::LANGUAGE_TSX.into()
        }
        "typescript" => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        "go" => tree_sitter_go::LANGUAGE.into(),
        "rust" => tree_sitter_rust::LANGUAGE.into(),
        "shell" => tree_sitter_bash::LANGUAGE.into(),
        _ => unreachable!(),
    };
    let mut parser = Parser::new();
    parser.set_language(&grammar)?;
    let mut progress = |_: &tree_sitter::ParseState| {
        if Instant::now() >= deadline {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    };
    let options = ParseOptions::new().progress_callback(&mut progress);
    let tree = parser
        .parse_with_options(&mut |offset, _| &bytes[offset..], None, Some(options))
        .context("Syntax parsing exceeded its budget")?;
    if tree.root_node().has_error() {
        bail!("Source syntax could not be parsed completely: {path}");
    }
    let mut result = Structure {
        language: language.into(),
        tests: Vec::new(),
        comments: Vec::new(),
        imports: Vec::new(),
        rust_use_arguments: Vec::new(),
        nonblank_lines: std::str::from_utf8(bytes)?
            .lines()
            .enumerate()
            .filter_map(|(index, line)| (!line.trim().is_empty()).then_some(index + 1))
            .collect(),
    };
    let mut pending = vec![tree.root_node()];
    let mut imports = Vec::new();
    let mut visited = 0;
    while let Some(node) = pending.pop() {
        check_deadline()?;
        visited += 1;
        if visited > 200_000 {
            bail!("Source syntax exceeds node budget: {path}");
        }
        if matches!(node.kind(), "comment" | "line_comment" | "block_comment") {
            result.comments.push(Comment {
                text: text(node, bytes).into(),
                range: range(node),
            });
        }
        // Go has a declaration wrapper around each spec, including a single
        // import. Capture only specs so grouped imports retain individual
        // locations and the wrapper cannot duplicate a finding.
        if (language == "go" && node.kind() == "import_spec")
            || (language != "go"
                && matches!(
                    node.kind(),
                    "import_declaration"
                        | "import_statement"
                        | "import_from_statement"
                        | "use_declaration"
                ))
        {
            imports.push(node);
        }
        if let Some(test) = tests_in_source::extract(language, path, node, bytes) {
            result.tests.push(test);
        }
        let mut cursor = node.walk();
        pending.extend(node.named_children(&mut cursor));
    }
    result.tests.sort_by_key(|test| test.range.start_line);
    imports.sort_by_key(|node| node.start_position().row);
    for node in imports {
        check_deadline()?;
        if language == "rust" {
            let argument = node
                .child_by_field_name("argument")
                .with_context(|| format!("Rust use argument syntax is unavailable: {path}"))?;
            let start = node.start_byte();
            result
                .rust_use_arguments
                .push(argument.start_byte() - start..argument.end_byte() - start);
        }
        result.imports.push(Import {
            text: if language == "go" {
                format!("import {}", text(node, bytes))
            } else {
                text(node, bytes).into()
            },
            range: range(node),
        });
    }
    result
        .comments
        .sort_by_key(|comment| comment.range.start_line);
    check_deadline()?;
    Ok(Some(result))
}

fn text<'a>(node: Node<'_>, bytes: &'a [u8]) -> &'a str {
    node.utf8_text(bytes).unwrap_or_default()
}
fn range(node: Node<'_>) -> Range {
    Range {
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
    }
}

#[cfg(test)]
mod tests;
