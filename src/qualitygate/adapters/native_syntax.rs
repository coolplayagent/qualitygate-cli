//! Bounded C/C++ syntax review. No name resolution or preprocessing is implied.
use super::{diagnostic_budget::BuiltinResult, rules::diagnostic};
use crate::{config::RuleSetting, domain::Range, snapshot::Snapshot};
use anyhow::{Context, Result, ensure};
use globset::{Glob, GlobSetBuilder};
use std::{
    ops::ControlFlow,
    time::{Duration, Instant},
};
use tree_sitter::{Node, ParseOptions, Parser, Tree};

const MAX_BYTES: usize = 4 * 1024 * 1024;
const MAX_NODES: usize = 200_000;

pub(super) fn evaluate(
    result: &mut BuiltinResult,
    setting: &RuleSetting,
    snapshot: &Snapshot,
) -> Result<()> {
    ensure!(
        matches!(
            result.id.as_str(),
            "c-no-atoi"
                | "c-no-goto"
                | "c-no-process-launch"
                | "cpp-no-reinterpret-cast"
                | "cpp-no-const-cast"
                | "cpp-no-delete-this"
                | "cpp-prefer-make-smart-pointer"
                | "cpp-use-nullptr"
                | "cpp-no-macro-constant"
                | "cpp-no-using-namespace-in-header"
        ),
        "Unsupported native syntax rule: {}",
        result.id
    );
    let languages: Vec<String> = serde_json::from_value(
        setting
            .parameters
            .get("languages")
            .context("Native syntax requires a fixed language scope")?
            .clone(),
    )?;
    let expected = if result.id.starts_with("c-") {
        vec!["c", "cpp"]
    } else {
        vec!["cpp"]
    };
    ensure!(
        languages == expected,
        "Native syntax requires languages: {expected:?}"
    );
    let paths: Vec<String> = setting
        .parameters
        .get("paths")
        .map(|value| serde_json::from_value(value.clone()))
        .transpose()?
        .unwrap_or_default();
    let mut filter = GlobSetBuilder::new();
    for path in paths {
        filter.add(Glob::new(&path)?);
    }
    let filter = filter.build()?;
    for (path, change) in &snapshot.changes {
        result.check_deadline()?;
        if change.kind == "deleted"
            || change.added_lines.is_empty()
            || !snapshot.includes(path)
            || (!filter.is_empty() && !filter.is_match(path))
        {
            continue;
        }
        let extension = path.rsplit('.').next().unwrap_or("");
        if result.id == "cpp-no-using-namespace-in-header"
            && !matches!(extension, "h" | "hh" | "hpp" | "hxx")
        {
            continue;
        }
        let language = match extension {
            "c" => "c",
            "h" | "cc" | "cpp" | "cxx" | "hh" | "hpp" | "hxx" => "cpp",
            _ => continue,
        };
        if !languages.iter().any(|item| item == language) {
            continue;
        }
        let file = snapshot
            .files
            .get(path)
            .with_context(|| format!("Changed native file is absent from snapshot: {path}"))?;
        let source = std::str::from_utf8(&file.bytes)
            .with_context(|| format!("Native source is not UTF-8: {path}"))?;
        let deadline = result
            .deadline()
            .min(Instant::now() + Duration::from_secs(2));
        let tree = parse(source.as_bytes(), language, deadline)
            .with_context(|| format!("Native syntax evidence incomplete: {path}"))?;
        let mut cursor = tree.walk();
        let mut visited = 0;
        loop {
            ensure!(
                Instant::now() < deadline,
                "Native syntax traversal exceeded its budget: {path}"
            );
            visited += 1;
            ensure!(
                visited <= MAX_NODES,
                "Native syntax exceeds {MAX_NODES} nodes: {path}"
            );
            let node = cursor.node();
            if matches_rule(&result.id, node, source.as_bytes(), deadline, &mut visited)? {
                // A smart-pointer finding belongs to the construction, not just
                // its allocation argument: changing the type must trigger review.
                let evidence_node = if result.id == "cpp-prefer-make-smart-pointer" {
                    smart_owner(node).expect("matched smart-pointer construction")
                } else {
                    node
                };
                let range = Range {
                    start_line: evidence_node.start_position().row + 1,
                    end_line: evidence_node.end_position().row
                        + usize::from(evidence_node.end_position().column != 0),
                };
                if changed_token(evidence_node, &change.added_lines, deadline, &mut visited)? {
                    result.matched_entities += 1;
                    let finding = diagnostic(
                        &result.id,
                        Some(path),
                        Some(range),
                        format!("Changed C/C++ syntax matches review rule {}", result.id),
                        serde_json::json!({"language":language,"node_kind":evidence_node.kind(),"analysis":"syntax","symbol_resolution":false}),
                        "Review the syntax in context; types, macro expansion and symbol ownership are not resolved",
                        &format!(
                            "{path}:{}:{}:{}",
                            node.start_byte(),
                            node.end_byte(),
                            node.kind()
                        ),
                    );
                    result.push(finding)?;
                }
            }
            // Do not interpret documentation or literal contents as executable code.
            if !matches!(
                node.kind(),
                "comment" | "string_literal" | "raw_string_literal" | "char_literal"
            ) && cursor.goto_first_child()
            {
                continue;
            }
            loop {
                if cursor.goto_next_sibling() {
                    break;
                }
                if !cursor.goto_parent() {
                    break;
                }
            }
            if cursor.node() == tree.root_node() {
                break;
            }
        }
    }
    result
        .metadata
        .insert("analysis".into(), serde_json::json!("native-syntax"));
    Ok(())
}

fn parse(bytes: &[u8], language: &str, deadline: Instant) -> Result<Tree> {
    ensure!(bytes.len() <= MAX_BYTES, "Native source exceeds 4 MiB");
    ensure!(
        Instant::now() < deadline,
        "Native syntax parsing exceeded its budget"
    );
    let mut parser = Parser::new();
    parser.set_language(&match language {
        "c" => tree_sitter_c::LANGUAGE.into(),
        _ => tree_sitter_cpp::LANGUAGE.into(),
    })?;
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
        .context("Native syntax parsing exceeded its budget")?;
    ensure!(
        Instant::now() < deadline,
        "Native syntax parsing exceeded its budget"
    );
    ensure!(
        !tree.root_node().has_error(),
        "Native source could not be parsed completely"
    );
    Ok(tree)
}

fn text<'a>(node: Node<'_>, bytes: &'a [u8]) -> &'a str {
    std::str::from_utf8(&bytes[node.byte_range()])
        .expect("syntax node boundaries preserve validated UTF-8")
}

fn matches_rule(
    id: &str,
    node: Node<'_>,
    bytes: &[u8],
    deadline: Instant,
    visited: &mut usize,
) -> Result<bool> {
    Ok(match id {
        "c-no-goto" => node.kind() == "goto_statement",
        "c-no-atoi" | "c-no-process-launch" if node.kind() == "call_expression" => node
            .child_by_field_name("function")
            .is_some_and(|function| {
                let name = call_name(function, bytes);
                if id == "c-no-atoi" {
                    matches!(name, Some("atoi" | "atol" | "atoll" | "atof"))
                } else {
                    matches!(
                        name,
                        Some(
                            "system"
                                | "popen"
                                | "execl"
                                | "execlp"
                                | "execle"
                                | "execv"
                                | "execve"
                                | "execvp"
                                | "execvpe"
                                | "WinExec"
                                | "CreateProcess"
                                | "CreateProcessA"
                                | "CreateProcessW"
                                | "ShellExecute"
                                | "ShellExecuteA"
                                | "ShellExecuteW"
                        )
                    )
                }
            }),
        "cpp-no-reinterpret-cast" | "cpp-no-const-cast" if node.kind() == "call_expression" => node
            .child_by_field_name("function")
            .is_some_and(|function| {
                function.kind() == "template_function"
                    && function.child_by_field_name("name").is_some_and(|name| {
                        text(name, bytes)
                            == if id == "cpp-no-const-cast" {
                                "const_cast"
                            } else {
                                "reinterpret_cast"
                            }
                    })
            }),
        "cpp-no-delete-this" if node.kind() == "delete_expression" => {
            first_named(node).is_some_and(|child| unwrap_parentheses(child).kind() == "this")
        }
        "cpp-use-nullptr" => node.kind() == "null" && text(node, bytes) == "NULL",
        "cpp-no-using-namespace-in-header" if node.kind() == "using_declaration" => {
            let mut cursor = node.walk();
            node.children(&mut cursor)
                .any(|child| child.kind() == "namespace")
        }
        "cpp-prefer-make-smart-pointer" if node.kind() == "new_expression" => {
            smart_pointer_new(node, bytes)
        }
        "cpp-no-macro-constant" if node.kind() == "preproc_def" => {
            if let Some(value) = node.child_by_field_name("value") {
                numeric_macro(text(value, bytes), deadline, visited)?
            } else {
                false
            }
        }
        _ => false,
    })
}

fn first_named(node: Node<'_>) -> Option<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| !child.is_extra())
}

fn unwrap_parentheses(mut node: Node<'_>) -> Node<'_> {
    // Node depth is bounded by the traversal budget; each step reduces depth.
    while node.kind() == "parenthesized_expression" {
        let Some(child) = first_named(node) else {
            break;
        };
        node = child;
    }
    node
}

fn without_global_scope(mut node: Node<'_>) -> Node<'_> {
    while node.kind() == "qualified_identifier" && node.child_by_field_name("scope").is_none() {
        let Some(name) = node.child_by_field_name("name") else {
            break;
        };
        node = name;
    }
    node
}

fn call_name<'a>(node: Node<'_>, bytes: &'a [u8]) -> Option<&'a str> {
    let node = without_global_scope(node);
    if node.kind() == "identifier" {
        return Some(text(node, bytes));
    }
    if node.kind() == "qualified_identifier" {
        let scope = node.child_by_field_name("scope")?;
        let name = node.child_by_field_name("name")?;
        if text(scope, bytes) == "std" && name.kind() == "identifier" {
            return Some(text(name, bytes));
        }
    }
    None
}

fn smart_type(node: Node<'_>, bytes: &[u8]) -> bool {
    let node = without_global_scope(node);
    if node.kind() != "qualified_identifier" {
        return false;
    }
    let Some(scope) = node.child_by_field_name("scope") else {
        return false;
    };
    let Some(name) = node.child_by_field_name("name") else {
        return false;
    };
    text(scope, bytes) == "std"
        && matches!(name.kind(), "template_type" | "template_function")
        && name
            .child_by_field_name("name")
            .is_some_and(|name| matches!(text(name, bytes), "unique_ptr" | "shared_ptr"))
}

fn smart_owner(node: Node<'_>) -> Option<Node<'_>> {
    let owner = node.parent()?.parent()?;
    if owner.kind() == "init_declarator" {
        owner.parent()
    } else {
        Some(owner)
    }
}

fn changed_token(
    node: Node<'_>,
    lines: &std::collections::BTreeSet<usize>,
    deadline: Instant,
    visited: &mut usize,
) -> Result<bool> {
    let mut cursor = node.walk();
    loop {
        ensure!(
            Instant::now() < deadline,
            "Native syntax selection exceeded its budget"
        );
        *visited += 1;
        ensure!(
            *visited <= MAX_NODES,
            "Native syntax exceeds {MAX_NODES} nodes"
        );
        let current = cursor.node();
        if current.kind() != "comment" {
            if cursor.goto_first_child() {
                continue;
            }
            let start = current.start_position().row + 1;
            let end = current.end_position().row + usize::from(current.end_position().column != 0);
            if start <= end && lines.range(start..=end).next().is_some() {
                return Ok(true);
            }
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                return Ok(false);
            }
        }
    }
}

fn smart_pointer_new(node: Node<'_>, bytes: &[u8]) -> bool {
    let Some(arguments) = node.parent() else {
        return false;
    };
    if !matches!(arguments.kind(), "argument_list" | "initializer_list") {
        return false;
    }
    let Some(owner) = arguments.parent() else {
        return false;
    };
    match owner.kind() {
        "call_expression" => owner
            .child_by_field_name("function")
            .is_some_and(|ty| smart_type(ty, bytes)),
        "compound_literal_expression" => owner
            .child_by_field_name("type")
            .is_some_and(|ty| smart_type(ty, bytes)),
        "init_declarator" => owner
            .parent()
            .and_then(|declaration| declaration.child_by_field_name("type"))
            .is_some_and(|ty| smart_type(ty, bytes)),
        _ => false,
    }
}

fn numeric_macro(value: &str, deadline: Instant, visited: &mut usize) -> Result<bool> {
    // Parse the replacement as an expression, rather than guessing literals with regex.
    // Nonexpression macros are simply outside this rule's numeric-literal scope.
    let source = format!("auto value = {value}\n;");
    let mut parser = Parser::new();
    parser.set_language(&tree_sitter_cpp::LANGUAGE.into())?;
    ensure!(
        Instant::now() < deadline,
        "Native macro parsing exceeded its budget"
    );
    let mut progress = |_: &tree_sitter::ParseState| {
        if Instant::now() >= deadline {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    };
    let tree = parser
        .parse_with_options(
            &mut |offset, _| &source.as_bytes()[offset..],
            None,
            Some(ParseOptions::new().progress_callback(&mut progress)),
        )
        .context("Native macro parsing exceeded its budget")?;
    ensure!(
        Instant::now() < deadline,
        "Native macro parsing exceeded its budget"
    );
    // Replacement expressions are a second syntax tree; charge them against
    // the same file-wide node budget before classifying the literal.
    let mut budget_cursor = tree.walk();
    loop {
        ensure!(
            Instant::now() < deadline,
            "Native macro traversal exceeded its budget"
        );
        *visited += 1;
        ensure!(
            *visited <= MAX_NODES,
            "Native syntax exceeds {MAX_NODES} nodes"
        );
        if budget_cursor.goto_first_child() {
            continue;
        }
        loop {
            if budget_cursor.goto_next_sibling() {
                break;
            }
            if !budget_cursor.goto_parent() {
                break;
            }
        }
        if budget_cursor.node() == tree.root_node() {
            break;
        }
    }
    if tree.root_node().has_error() {
        return Ok(false);
    }
    let mut cursor = tree.root_node().walk();
    if tree
        .root_node()
        .named_children(&mut cursor)
        .filter(|child| !child.is_extra())
        .count()
        != 1
    {
        return Ok(false);
    }
    let Some(declaration) = first_named(tree.root_node()) else {
        return Ok(false);
    };
    let mut cursor = declaration.walk();
    if declaration
        .children_by_field_name("declarator", &mut cursor)
        .count()
        != 1
    {
        return Ok(false);
    }
    let Some(initializer) = declaration.child_by_field_name("declarator") else {
        return Ok(false);
    };
    let Some(mut value) = initializer.child_by_field_name("value") else {
        return Ok(false);
    };
    loop {
        ensure!(
            Instant::now() < deadline,
            "Native macro traversal exceeded its budget"
        );
        value = unwrap_parentheses(value);
        if value.kind() == "number_literal" {
            return Ok(true);
        }
        if value.kind() != "unary_expression" {
            return Ok(false);
        }
        if !value
            .child_by_field_name("operator")
            .is_some_and(|op| matches!(text(op, source.as_bytes()), "+" | "-"))
        {
            return Ok(false);
        }
        let Some(argument) = value.child_by_field_name("argument") else {
            return Ok(false);
        };
        value = argument;
    }
}

#[cfg(test)]
#[path = "native_syntax_tests.rs"]
mod tests;
