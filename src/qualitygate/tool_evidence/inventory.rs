//! Independently inventories the supported Rust function shape in captured bytes.

use super::contracts::{
    Inventory, InventoryEntry, Location, MAX_RECORDS, validate_path, validate_text,
};
use anyhow::{Context, Result, bail, ensure};
use std::{
    collections::{BTreeMap, BTreeSet},
    ops::ControlFlow,
    time::{Duration, Instant},
};
use tree_sitter::{Node, ParseOptions, Parser};

pub fn expected_inventory(
    spec: &super::contracts::OutputSpec,
    sources: &BTreeMap<String, Vec<u8>>,
) -> Result<Inventory> {
    spec.validate()?;
    let requested: BTreeSet<_> = spec.paths.iter().collect();
    let actual: BTreeSet<_> = sources.keys().collect();
    ensure!(
        requested == actual,
        "Snapshot sources do not match declared tool inventory"
    );
    rust_inventory(sources, &spec.crate_root, &spec.crate_name)
}

/// `crate_root` is repository-relative (empty for the repository root). Inputs
/// must already come from the selected snapshot; this function performs no I/O.
pub fn rust_inventory(
    sources: &BTreeMap<String, Vec<u8>>,
    crate_root: &str,
    crate_name: &str,
) -> Result<Inventory> {
    validate_text(crate_name)?;
    ensure!(
        crate_name
            .bytes()
            .all(|v| v.is_ascii_alphanumeric() || v == b'_'),
        "Invalid Rust crate name"
    );
    if !crate_root.is_empty() {
        validate_path(crate_root)?;
    }
    ensure!(
        sources.len() <= MAX_RECORDS,
        "Source file inventory exceeds bound"
    );
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut parser = Parser::new();
    parser.set_language(&tree_sitter_rust::LANGUAGE.into())?;
    let mut entities = Vec::new();
    let mut identity = BTreeSet::new();
    let mut work = 0usize;
    let mut total_bytes = 0usize;
    for (file, bytes) in sources {
        validate_path(file)?;
        ensure!(
            file.ends_with(".rs"),
            "Native tool inventory supports Rust source files only"
        );
        total_bytes = total_bytes
            .checked_add(bytes.len())
            .context("Source byte count overflow")?;
        ensure!(
            total_bytes <= 32 * 1024 * 1024,
            "Source inventory exceeds 32 MiB"
        );
        std::str::from_utf8(bytes).context("Source inventory is not UTF-8")?;
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
            .context("Tool inventory parsing exceeded deadline")?;
        ensure!(
            !tree.root_node().has_error(),
            "Incomplete Rust syntax in tool scope: {file}"
        );
        let modules = file_modules(file, crate_root, crate_name)?;
        let mut pending = vec![tree.root_node()];
        while let Some(node) = pending.pop() {
            work += 1;
            ensure!(
                work <= 500_000 && Instant::now() < deadline,
                "Tool inventory work budget exceeded"
            );
            if node.kind() == "function_item" && node.child_by_field_name("body").is_some() {
                let mods = ancestor_modules(node, bytes)?;
                if !mods.iter().any(|name| name == "tests")
                    && node.parent().is_none_or(|p| p.kind() != "block")
                {
                    let name_node = node
                        .child_by_field_name("name")
                        .context("Rust function has no name")?;
                    let name = text(name_node, bytes)?.to_string();
                    let mut namespace = modules.clone();
                    namespace.extend(mods);
                    if let Some(owner) = impl_owner(node, bytes)? {
                        namespace.push(owner);
                    }
                    let namespace = namespace.join("::");
                    ensure!(
                        identity.insert((namespace.clone(), name.clone())),
                        "Ambiguous upstream namespace/function identity: {namespace}::{name}"
                    );
                    ensure!(
                        entities.len() < MAX_RECORDS,
                        "Function inventory exceeds bound"
                    );
                    entities.push(InventoryEntry {
                        namespace: namespace.clone(),
                        name: name.clone(),
                        location: Location {
                            file: file.clone(),
                            start_line: node.start_position().row + 1,
                            end_line: node.end_position().row
                                + usize::from(node.end_position().column != 0),
                            symbol: format!("{namespace}::{name}"),
                        },
                    });
                }
            }
            let mut cursor = node.walk();
            pending.extend(node.named_children(&mut cursor));
        }
    }
    entities.sort_by(|a, b| a.location.cmp(&b.location));
    Ok(Inventory {
        files: sources.keys().cloned().collect(),
        entities,
    })
}

fn text<'a>(node: Node<'_>, bytes: &'a [u8]) -> Result<&'a str> {
    Ok(std::str::from_utf8(&bytes[node.byte_range()])?)
}

fn file_modules(file: &str, root: &str, name: &str) -> Result<Vec<String>> {
    let relative = if root.is_empty() {
        file
    } else {
        file.strip_prefix(&format!("{root}/"))
            .context("Source is outside its declared crate root")?
    };
    let relative = relative.strip_prefix("src/").unwrap_or(relative);
    if matches!(relative, "lib.rs" | "main.rs") {
        return Ok(vec![name.to_string()]);
    }
    let mut parts: Vec<String> = relative.split('/').map(str::to_owned).collect();
    let last = parts.pop().context("Missing file name")?;
    if last != "mod.rs" {
        parts.push(
            last.strip_suffix(".rs")
                .context("Expected Rust file")?
                .to_string(),
        );
    }
    if parts.first().is_some_and(|v| v == "bin") && parts.len() >= 2 {
        parts.remove(0);
    } else {
        parts.insert(0, name.to_string());
    }
    Ok(parts)
}

fn ancestor_modules(node: Node<'_>, bytes: &[u8]) -> Result<Vec<String>> {
    let mut result = Vec::new();
    let mut parent = node.parent();
    while let Some(node) = parent {
        if node.kind() == "mod_item" {
            result.push(
                text(
                    node.child_by_field_name("name").context("Unnamed module")?,
                    bytes,
                )?
                .to_string(),
            );
        }
        parent = node.parent();
    }
    result.reverse();
    Ok(result)
}

fn impl_owner(node: Node<'_>, bytes: &[u8]) -> Result<Option<String>> {
    let Some(parent) = node
        .parent()
        .and_then(|p| p.parent())
        .filter(|p| p.kind() == "impl_item")
    else {
        return Ok(None);
    };
    let owner = parent
        .child_by_field_name("type")
        .context("Impl has no self type")?;
    // Only names whose upstream representation is unambiguous are accepted.
    if owner.kind() != "type_identifier" {
        bail!("Native tool inventory requires an unambiguous simple impl self type");
    }
    Ok(Some(text(owner, bytes)?.to_string()))
}

#[cfg(test)]
#[path = "inventory_tests.rs"]
mod tests;
