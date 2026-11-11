//! Structured review signals for Python, Go, Shell and TypeScript/JavaScript.
use super::{
    diagnostic_budget::BuiltinResult,
    language_bindings::{Go, TypeScript},
    language_shell,
    rules::diagnostic,
    syntax::review::{self, View},
};
use crate::{config::RuleSetting, domain::Range, snapshot::Snapshot};
use anyhow::{Context, Result, ensure};
use globset::{Glob, GlobSetBuilder};
use std::{
    ops::Range as Bytes,
    time::{Duration, Instant},
};
use tree_sitter::Node;

fn language(id: &str) -> Result<&'static str> {
    Ok(match id {
        "py-none-comparison"
        | "py-walrus-operator"
        | "py-lambda-assignment"
        | "py-sql-string-format" => "python",
        "go-unsafe-package" | "go-os-exit" | "go-import-alias" => "go",
        "shell-eval"
        | "shell-alias"
        | "shell-chmod-read"
        | "shell-trap-uncapturable"
        | "shell-assignment-spaces"
        | "shell-comparison-spaces" => "shell",
        "ts-no-with" | "ts-no-var" | "ts-no-arguments" => "typescript",
        _ => anyhow::bail!("Unsupported language syntax rule: {id}"),
    })
}

pub(super) fn evaluate(
    result: &mut BuiltinResult,
    setting: &RuleSetting,
    snapshot: &Snapshot,
) -> Result<()> {
    let expected = language(&result.id)?;
    ensure!(
        setting.parameters.get("languages") == Some(&serde_json::json!([expected])),
        "{} requires languages: [{expected}]",
        result.id
    );
    let paths: Vec<String> = setting
        .parameters
        .get("paths")
        .map(|paths| serde_json::from_value(paths.clone()))
        .transpose()?
        .unwrap_or_default();
    let mut paths_filter = GlobSetBuilder::new();
    for path in paths {
        paths_filter.add(Glob::new(&path)?);
    }
    let paths_filter = paths_filter.build()?;
    for (path, change) in &snapshot.changes {
        result.check_deadline()?;
        if change.kind == "deleted"
            || change.added_lines.is_empty()
            || !snapshot.includes(path)
            || (!paths_filter.is_empty() && !paths_filter.is_match(path))
        {
            continue;
        }
        let file = snapshot
            .files
            .get(path)
            .with_context(|| format!("Changed file is absent from snapshot: {path}"))?;
        if crate::domain::language::for_source(path, &file.bytes)
            .is_none_or(|language| language.name != expected)
        {
            continue;
        }
        let deadline = result
            .deadline()
            .min(Instant::now() + Duration::from_secs(2));
        let (tree, work) = if result.id == "shell-comparison-spaces" {
            review::parse_shell_comparisons(&file.bytes, path, deadline)?
        } else {
            (review::parse(&file.bytes, expected, path, deadline)?, 0)
        };
        let view = View::new_with_work(&tree, &file.bytes, &change.added_lines, deadline, work)?;
        let go = matches!(result.id.as_str(), "go-os-exit" | "go-unsafe-package")
            .then(|| Go::new(&view))
            .transpose()?;
        let typescript = (result.id == "ts-no-arguments")
            .then(|| TypeScript::new(&view))
            .transpose()?;
        for &node in &view.nodes {
            view.charge()?;
            let Some(focus) = signal(&result.id, node, &view, go.as_ref(), typescript.as_ref())?
            else {
                continue;
            };
            if !view.changed(focus) {
                continue;
            }
            result.matched_entities += 1;
            result.push(diagnostic(&result.id, Some(path), Some(Range {
                start_line: node.start_position().row + 1,
                end_line: node.end_position().row + usize::from(node.end_position().column != 0),
            }), format!("Changed syntax matches review rule {}", result.id),
            serde_json::json!({"language":expected,"node_kind":node.kind(),"analysis":"syntax","resolution":"file-local lexical bindings only"}),
            "Review this syntax in context; runtime values, cross-file types and dynamic commands are not resolved",
            &format!("{path}:{}:{}:{}", node.start_byte(), node.end_byte(), node.kind())))?;
        }
    }
    result
        .metadata
        .insert("analysis".into(), serde_json::json!("language-syntax"));
    Ok(())
}

fn signal<'a>(
    id: &str,
    node: Node<'a>,
    view: &View<'a>,
    go: Option<&Go>,
    typescript: Option<&TypeScript>,
) -> Result<Option<Bytes<usize>>> {
    let found = match id {
        "py-none-comparison" if node.kind() == "comparison_operator" => {
            let operands = view.children(node)?;
            let mut cursor = node.walk();
            let mut found = false;
            for (index, operator) in node
                .children_by_field_name("operators", &mut cursor)
                .enumerate()
            {
                view.charge()?;
                if matches!(operator.kind(), "==" | "!=") {
                    for operand in operands.get(index..=index + 1).unwrap_or_default() {
                        found |= view.unparen(*operand)?.kind() == "none";
                    }
                }
            }
            found
        }
        "py-walrus-operator" => node.kind() == "named_expression",
        "py-sql-string-format" => sql_format(node, view)?,
        "py-lambda-assignment" if node.kind() == "assignment" => {
            if let (Some(left), Some(right)) = (
                node.child_by_field_name("left"),
                node.child_by_field_name("right"),
            ) {
                left.kind() == "identifier" && view.unparen(right)?.kind() == "lambda"
            } else {
                false
            }
        }
        "go-import-alias" => {
            node.kind() == "import_spec"
                && node
                    .child_by_field_name("name")
                    .is_some_and(|name| name.kind() == "package_identifier")
        }
        "go-unsafe-package" | "go-os-exit" => go
            .context("Go binding evidence is absent")?
            .matches(id, node),
        "ts-no-with" if node.kind() == "with_statement" => {
            let body = node
                .child_by_field_name("body")
                .context("With statement body is absent")?;
            return Ok(Some(node.start_byte()..body.start_byte()));
        }
        "ts-no-var" => {
            if node.kind() == "for_in_statement"
                && node
                    .child_by_field_name("kind")
                    .is_some_and(|kind| kind.kind() == "var")
            {
                let body = node
                    .child_by_field_name("body")
                    .context("For statement body is absent")?;
                return Ok(Some(node.start_byte()..body.start_byte()));
            }
            node.kind() == "variable_declaration"
        }
        "ts-no-arguments" => typescript
            .context("TypeScript binding evidence is absent")?
            .implicit_arguments(node, view)?,
        "shell-eval"
        | "shell-alias"
        | "shell-chmod-read"
        | "shell-trap-uncapturable"
        | "shell-assignment-spaces"
        | "shell-comparison-spaces" => language_shell::matches(id, node, view)?,
        _ => false,
    };
    Ok(found.then(|| node.byte_range()))
}

fn sql_format(node: Node<'_>, view: &View<'_>) -> Result<bool> {
    if node.kind() != "call" {
        return Ok(false);
    }
    let Some(function) = node.child_by_field_name("function") else {
        return Ok(false);
    };
    let name = if function.kind() == "attribute" {
        function.child_by_field_name("attribute")
    } else {
        Some(function)
    };
    if !name.is_some_and(|name| matches!(view.text(name), "execute" | "executemany")) {
        return Ok(false);
    }
    let Some(arguments) = node.child_by_field_name("arguments") else {
        return Ok(false);
    };
    let Some(argument) = view.children(arguments)?.first().copied() else {
        return Ok(false);
    };
    let argument = view.unparen(argument)?;
    Ok(match argument.kind() {
        "binary_operator" => argument
            .child_by_field_name("operator")
            .is_some_and(|operator| operator.kind() == "%"),
        "call" => argument
            .child_by_field_name("function")
            .is_some_and(|function| {
                function.kind() == "attribute"
                    && function
                        .child_by_field_name("attribute")
                        .is_some_and(|name| view.text(name) == "format")
            }),
        "string" | "concatenated_string" => {
            let mut pending = vec![argument];
            let mut interpolated = false;
            while let Some(part) = pending.pop() {
                view.charge()?;
                interpolated |= part.kind() == "interpolation";
                pending.extend(view.children(part)?);
            }
            interpolated
        }
        _ => false,
    })
}

#[cfg(test)]
#[path = "language_syntax_tests.rs"]
mod tests;
