//! Framework-aware syntax facts from bounded Tree-sitter parsing.

mod tests_in_source;

use crate::domain::Range;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    ops::ControlFlow,
    path::Path,
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
}

pub fn language(path: &str) -> Option<&'static str> {
    match Path::new(path).extension()?.to_str()? {
        "java" => Some("java"),
        "py" => Some("python"),
        "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" => Some("typescript"),
        "go" => Some("go"),
        "rs" => Some("rust"),
        "sh" | "bash" => Some("shell"),
        _ => None,
    }
}

pub fn parse(path: &str, bytes: &[u8]) -> Result<Option<Structure>> {
    let Some(language) = language(path) else {
        return Ok(None);
    };
    std::str::from_utf8(bytes).context("Source file is not UTF-8")?;
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
    let started = Instant::now();
    let mut progress = |_: &tree_sitter::ParseState| {
        if started.elapsed() > Duration::from_secs(2) {
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
    };
    let mut pending = vec![tree.root_node()];
    let mut visited = 0;
    while let Some(node) = pending.pop() {
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
        if matches!(
            node.kind(),
            "import_declaration"
                | "import_statement"
                | "import_from_statement"
                | "import_spec"
                | "use_declaration"
        ) {
            result.imports.push(Import {
                text: text(node, bytes).into(),
                range: range(node),
            });
        }
        if let Some(test) = tests_in_source::extract(language, path, node, bytes) {
            result.tests.push(test);
        }
        let mut cursor = node.walk();
        pending.extend(node.named_children(&mut cursor));
    }
    result.tests.sort_by_key(|test| test.range.start_line);
    result
        .comments
        .sort_by_key(|comment| comment.range.start_line);
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
#[path = "syntax_tests.rs"]
mod tests;
