//! Bounded syntax evidence for language review signals, including executable interpolations.
use anyhow::{Context, Result, ensure};
use std::{
    cell::Cell,
    collections::BTreeSet,
    ops::{ControlFlow, Range},
    time::Instant,
};
use tree_sitter::{Language, Node, ParseOptions, Parser, Tree};

pub(crate) const MAX_BYTES: usize = 4 * 1024 * 1024;
pub(crate) const MAX_WORK: usize = 200_000;

pub(crate) fn parse(bytes: &[u8], language: &str, path: &str, deadline: Instant) -> Result<Tree> {
    ensure!(
        bytes.len() <= MAX_BYTES,
        "Review source exceeds 4 MiB: {path}"
    );
    std::str::from_utf8(bytes).context("Review source is not UTF-8")?;
    ensure!(
        Instant::now() < deadline,
        "Syntax review exceeded its deadline"
    );
    let grammar: Language = match language {
        "python" => tree_sitter_python::LANGUAGE.into(),
        "go" => tree_sitter_go::LANGUAGE.into(),
        "shell" => tree_sitter_bash::LANGUAGE.into(),
        "typescript" if path.ends_with(".tsx") || path.ends_with(".jsx") => {
            tree_sitter_typescript::LANGUAGE_TSX.into()
        }
        "typescript" => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        _ => anyhow::bail!("Unsupported syntax review language: {language}"),
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
    let tree = parser
        .parse_with_options(
            &mut |offset, _| &bytes[offset..],
            None,
            Some(ParseOptions::new().progress_callback(&mut progress)),
        )
        .context("Syntax review exceeded its deadline")?;
    ensure!(
        Instant::now() < deadline,
        "Syntax review exceeded its deadline"
    );
    ensure!(
        !tree.root_node().has_error(),
        "Source syntax could not be parsed completely: {path}"
    );
    Ok(tree)
}

pub(crate) struct View<'a> {
    pub nodes: Vec<Node<'a>>,
    pub bytes: &'a [u8],
    changed: Vec<Range<usize>>,
    deadline: Instant,
    work: Cell<usize>,
}

impl<'a> View<'a> {
    pub fn new(
        tree: &'a Tree,
        bytes: &'a [u8],
        lines: &BTreeSet<usize>,
        deadline: Instant,
    ) -> Result<Self> {
        let mut view = Self {
            nodes: Vec::new(),
            bytes,
            changed: Vec::new(),
            deadline,
            work: Cell::new(0),
        };
        let mut cursor = tree.walk();
        loop {
            view.charge()?;
            let node = cursor.node();
            if node.kind() != "comment" {
                view.nodes.push(node);
                if node.child_count() == 0 {
                    let start = node.start_position().row + 1;
                    let end =
                        node.end_position().row + usize::from(node.end_position().column != 0);
                    if start <= end
                        && lines.range(start..=end).next().is_some()
                        && node.start_byte() < node.end_byte()
                    {
                        view.changed.push(node.byte_range());
                    }
                }
                // Do not skip strings: interpolation/substitution nodes contain real code.
                if cursor.goto_first_child() {
                    continue;
                }
            }
            loop {
                if cursor.goto_next_sibling() {
                    break;
                }
                if !cursor.goto_parent() {
                    return Ok(view);
                }
            }
        }
    }

    pub fn charge(&self) -> Result<()> {
        ensure!(
            Instant::now() < self.deadline,
            "Syntax review exceeded its deadline"
        );
        let work = self.work.get() + 1;
        self.work.set(work);
        ensure!(
            work <= MAX_WORK,
            "Syntax review exceeds {MAX_WORK} node/work steps"
        );
        Ok(())
    }

    pub fn text(&self, node: Node<'_>) -> &'a str {
        std::str::from_utf8(&self.bytes[node.byte_range()])
            .expect("validated UTF-8 syntax boundaries")
    }

    pub fn changed(&self, range: Range<usize>) -> bool {
        let index = self
            .changed
            .partition_point(|token| token.end <= range.start);
        self.changed
            .get(index)
            .is_some_and(|token| token.start < range.end)
    }

    pub fn children(&self, node: Node<'a>) -> Result<Vec<Node<'a>>> {
        let mut cursor = node.walk();
        let mut children = Vec::new();
        for child in node.named_children(&mut cursor) {
            self.charge()?;
            if !child.is_extra() {
                children.push(child);
            }
        }
        Ok(children)
    }

    pub fn ancestor(&self, mut node: Node<'a>, kinds: &[&str]) -> Result<Option<Node<'a>>> {
        while let Some(parent) = node.parent() {
            self.charge()?;
            if kinds.contains(&parent.kind()) {
                return Ok(Some(parent));
            }
            node = parent;
        }
        Ok(None)
    }

    pub fn unparen(&self, mut node: Node<'a>) -> Result<Node<'a>> {
        while node.kind() == "parenthesized_expression" {
            self.charge()?;
            let Some(child) = self.children(node)?.first().copied() else {
                break;
            };
            node = child;
        }
        Ok(node)
    }
}
