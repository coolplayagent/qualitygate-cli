//! File-local bindings for Go package references and JavaScript's implicit arguments.
use crate::source_analysis::syntax::review::View;
use anyhow::{Result, ensure};
use std::{
    collections::{BTreeMap, BTreeSet},
    ops::Range,
};
use tree_sitter::Node;

struct Binding {
    scope: Range<usize>,
    from: usize,
}
#[derive(Default)]
struct Bindings {
    names: BTreeMap<String, Vec<Binding>>,
    declarations: BTreeSet<usize>,
}
impl Bindings {
    fn add(&mut self, view: &View<'_>, name: Node<'_>, scope: Node<'_>, from: usize) {
        self.names
            .entry(view.text(name).into())
            .or_default()
            .push(Binding {
                scope: scope.byte_range(),
                from,
            });
        self.declarations.insert(name.id());
    }
    fn shadows(
        &self,
        view: &View<'_>,
        name: &str,
        position: usize,
        inside: Option<Range<usize>>,
    ) -> Result<bool> {
        for binding in self.names.get(name).into_iter().flatten() {
            view.charge()?;
            if binding.from <= position
                && binding.scope.contains(&position)
                && inside.as_ref().is_none_or(|scope| {
                    scope.start <= binding.scope.start && binding.scope.end <= scope.end
                })
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

struct Import<'a> {
    path: String,
    node: Node<'a>,
}
#[derive(Default)]
struct Imports<'a> {
    named: BTreeMap<String, Import<'a>>,
    dot: BTreeMap<String, Import<'a>>,
    unsafe_nodes: BTreeSet<usize>,
}
const UNSAFE_NAMES: &[&str] = &[
    "Pointer",
    "Sizeof",
    "Offsetof",
    "Alignof",
    "Add",
    "Slice",
    "SliceData",
    "String",
    "StringData",
];
pub(crate) struct Go {
    exits: BTreeSet<usize>,
    unsafe_uses: BTreeSet<usize>,
}
const GO_SCOPES: &[&str] = &[
    "block",
    "if_statement",
    "for_statement",
    "expression_switch_statement",
    "type_switch_statement",
    "communication_case",
    "expression_case",
    "type_case",
    "source_file",
];
const GO_FUNCTIONS: &[&str] = &[
    "function_declaration",
    "method_declaration",
    "func_literal",
    "function_type",
];

impl<'a> Imports<'a> {
    fn reference<'s>(
        &'s self,
        node: Node<'a>,
        view: &View<'a>,
        bindings: &Bindings,
    ) -> Result<Option<(&'a str, &'s Import<'a>)>> {
        let (alias, member, import) = match node.kind() {
            "selector_expression" | "qualified_type" => {
                let package = node
                    .child_by_field_name("operand")
                    .or_else(|| node.child_by_field_name("package"));
                let member = node
                    .child_by_field_name("field")
                    .or_else(|| node.child_by_field_name("name"));
                let (Some(package), Some(member)) = (package, member) else {
                    return Ok(None);
                };
                if !matches!(package.kind(), "identifier" | "package_identifier") {
                    return Ok(None);
                }
                let alias = view.text(package);
                (alias, view.text(member), self.named.get(alias))
            }
            "identifier" | "type_identifier" => {
                if bindings.declarations.contains(&node.id()) {
                    return Ok(None);
                }
                if node.parent().is_some_and(|parent| {
                    parent.kind() == "qualified_type"
                        && parent.child_by_field_name("name") == Some(node)
                }) {
                    return Ok(None);
                }
                // A bare keyed-literal name may be a struct field, not a dot-import reference.
                if let Some(parent) = node.parent()
                    && parent.kind() == "literal_element"
                    && parent.parent().is_some_and(|owner| {
                        owner.kind() == "keyed_element"
                            && owner.child_by_field_name("key") == Some(parent)
                    })
                {
                    return Ok(None);
                }
                let member = view.text(node);
                let path = match member {
                    "Exit" => "os",
                    "Goexit" => "runtime",
                    _ if UNSAFE_NAMES.contains(&member) => "unsafe",
                    _ => return Ok(None),
                };
                (member, member, self.dot.get(path))
            }
            _ => return Ok(None),
        };
        if bindings.shadows(view, alias, node.start_byte(), None)? {
            return Ok(None);
        }
        Ok(import.map(|import| (member, import)))
    }
}

impl Go {
    pub fn new(view: &View<'_>) -> Result<Self> {
        let mut imports = Imports::default();
        for &node in &view.nodes {
            view.charge()?;
            if node.kind() != "import_spec" {
                continue;
            }
            let Some(path) = node.child_by_field_name("path") else {
                continue;
            };
            let Some(path) = go_string(view.text(path)) else {
                anyhow::bail!("Go import string could not be decoded");
            };
            if !matches!(path.as_str(), "os" | "runtime" | "unsafe") {
                continue;
            }
            if path == "unsafe" {
                imports.unsafe_nodes.insert(node.id());
            }
            let alias = node
                .child_by_field_name("name")
                .map(|name| view.text(name))
                .unwrap_or(&path)
                .to_owned();
            let import = Import { path, node };
            match alias.as_str() {
                "_" => (),
                "." => {
                    ensure!(
                        imports.dot.insert(import.path.clone(), import).is_none(),
                        "Ambiguous Go dot import bindings"
                    );
                }
                _ => {
                    ensure!(
                        imports.named.insert(alias, import).is_none(),
                        "Ambiguous Go package import bindings"
                    );
                }
            }
        }
        let mut relevant: BTreeSet<_> = imports.named.keys().map(String::as_str).collect();
        relevant.extend(["Exit", "Goexit"]);
        relevant.extend(UNSAFE_NAMES.iter().copied());
        let mut bindings = Bindings::default();
        for &node in &view.nodes {
            view.charge()?;
            let (names, scope, from) = match node.kind() {
                "parameter_declaration" | "variadic_parameter_declaration" => {
                    let scope = view
                        .ancestor(node, GO_FUNCTIONS)?
                        .and_then(|owner| owner.child_by_field_name("body"));
                    (
                        named_fields(view, node, "name")?,
                        scope,
                        scope.map_or(0, |scope| scope.start_byte()),
                    )
                }
                "var_spec" | "const_spec" | "type_spec" | "type_alias" => (
                    named_fields(view, node, "name")?,
                    view.ancestor(node, GO_SCOPES)?,
                    node.end_byte(),
                ),
                "type_switch_statement" => {
                    if let Some(alias) = node.child_by_field_name("alias") {
                        let names = view.children(alias)?;
                        for case in view.children(node)? {
                            if !matches!(case.kind(), "type_case" | "default_case") {
                                continue;
                            }
                            let body = view
                                .children(case)?
                                .into_iter()
                                .find(|child| child.kind() == "statement_list");
                            if let Some(body) = body {
                                for &name in &names {
                                    if relevant.contains(view.text(name)) {
                                        bindings.add(view, name, case, body.start_byte());
                                    }
                                }
                            }
                        }
                    }
                    continue;
                }
                "short_var_declaration" | "range_clause" | "receive_statement" => {
                    let mut cursor = node.walk();
                    let mut declares = false;
                    for child in node.children(&mut cursor) {
                        view.charge()?;
                        declares |= child.kind() == ":=";
                    }
                    if !declares {
                        continue;
                    }
                    let names = match node.child_by_field_name("left") {
                        Some(left) => view.children(left)?,
                        None => Vec::new(),
                    };
                    (names, view.ancestor(node, GO_SCOPES)?, node.end_byte())
                }
                _ => continue,
            };
            if let Some(scope) = scope {
                for name in names {
                    if relevant.contains(view.text(name)) {
                        bindings.add(view, name, scope, from);
                    }
                }
            }
        }
        let mut exits = BTreeSet::new();
        let mut unsafe_uses = imports.unsafe_nodes.clone();
        for &node in &view.nodes {
            view.charge()?;
            if let Some((_, import)) = imports.reference(node, view, &bindings)?
                && import.path == "unsafe"
                && !view.changed(import.node.byte_range())
            {
                unsafe_uses.insert(node.id());
            }
            if node.kind() == "call_expression"
                && let Some(function) = node.child_by_field_name("function")
                && let Some((member, import)) =
                    imports.reference(view.unparen(function)?, view, &bindings)?
                && matches!(
                    (import.path.as_str(), member),
                    ("os", "Exit") | ("runtime", "Goexit")
                )
            {
                // A changed import changes the meaning of retained calls. Attach
                // the review to that changed import so delivery filtering retains it.
                exits.insert(if view.changed(import.node.byte_range()) {
                    import.node.id()
                } else {
                    node.id()
                });
            }
        }
        Ok(Self { exits, unsafe_uses })
    }

    pub fn matches(&self, id: &str, node: Node<'_>) -> bool {
        match id {
            "go-os-exit" => self.exits.contains(&node.id()),
            "go-unsafe-package" => self.unsafe_uses.contains(&node.id()),
            _ => false,
        }
    }
}

fn named_fields<'a>(view: &View<'a>, node: Node<'a>, field: &str) -> Result<Vec<Node<'a>>> {
    let mut cursor = node.walk();
    let mut result = Vec::new();
    for name in node.children_by_field_name(field, &mut cursor) {
        view.charge()?;
        if name.is_named() {
            result.push(name);
        }
    }
    Ok(result)
}

fn go_string(value: &str) -> Option<String> {
    if let Some(raw) = value
        .strip_prefix('`')
        .and_then(|value| value.strip_suffix('`'))
    {
        return Some(raw.replace('\r', ""));
    }
    let value = value.strip_prefix('"')?.strip_suffix('"')?;
    let mut chars = value.chars();
    let mut result = String::new();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            result.push(ch);
            continue;
        }
        let escape = chars.next()?;
        let decoded = match escape {
            'a' => '\x07',
            'b' => '\x08',
            'f' => '\x0c',
            'n' => '\n',
            'r' => '\r',
            't' => '\t',
            'v' => '\x0b',
            '\\' => '\\',
            '"' => '"',
            'x' | 'u' | 'U' | '0'..='7' => {
                let (radix, count, mut digits) = match escape {
                    'x' => (16, 2, String::new()),
                    'u' => (16, 4, String::new()),
                    'U' => (16, 8, String::new()),
                    _ => (8, 2, escape.to_string()),
                };
                for _ in 0..count {
                    digits.push(chars.next()?);
                }
                char::from_u32(u32::from_str_radix(&digits, radix).ok()?)?
            }
            _ => return None,
        };
        result.push(decoded);
    }
    Some(result)
}

const TS_FUNCTIONS: &[&str] = &[
    "function_declaration",
    "function_expression",
    "generator_function_declaration",
    "generator_function",
    "method_definition",
    "arrow_function",
];
const TS_PARAMETERS: &[&str] = &[
    "function_declaration",
    "function_expression",
    "generator_function_declaration",
    "generator_function",
    "method_definition",
    "arrow_function",
    "function_type",
    "constructor_type",
    "call_signature",
    "construct_signature",
    "method_signature",
    "function_signature",
    "abstract_method_signature",
];
const TS_SCOPES: &[&str] = &[
    "statement_block",
    "for_statement",
    "for_in_statement",
    "switch_body",
    "catch_clause",
    "program",
];
pub(crate) struct TypeScript {
    bindings: Bindings,
}
impl TypeScript {
    pub fn new(view: &View<'_>) -> Result<Self> {
        let mut bindings = Bindings::default();
        for &node in &view.nodes {
            view.charge()?;
            if matches!(node.kind(), "function_expression" | "generator_function")
                && let Some(name) = node.child_by_field_name("name")
            {
                bindings.declarations.insert(name.id());
            }
            let (pattern, scope) = match node.kind() {
                "for_in_statement" if node.child_by_field_name("kind").is_some() => {
                    let is_var = node
                        .child_by_field_name("kind")
                        .is_some_and(|kind| kind.kind() == "var");
                    let scope = if is_var {
                        view.ancestor(node, TS_FUNCTIONS)?
                            .or(view.ancestor(node, &["program"])?)
                    } else {
                        Some(node)
                    };
                    (node.child_by_field_name("left"), scope)
                }
                "required_parameter" | "optional_parameter" => (
                    node.child_by_field_name("pattern")
                        .or_else(|| node.child_by_field_name("name")),
                    view.ancestor(node, TS_PARAMETERS)?
                        .filter(|owner| owner.child_by_field_name("body").is_some()),
                ),
                "arrow_function" => (node.child_by_field_name("parameter"), Some(node)),
                "variable_declarator" => {
                    let is_var = node
                        .parent()
                        .is_some_and(|parent| parent.kind() == "variable_declaration");
                    let scope = if is_var {
                        view.ancestor(node, TS_FUNCTIONS)?
                            .or(view.ancestor(node, &["program"])?)
                    } else {
                        view.ancestor(node, TS_SCOPES)?
                    };
                    (node.child_by_field_name("name"), scope)
                }
                "catch_clause" => (node.child_by_field_name("parameter"), Some(node)),
                "function_declaration"
                | "generator_function_declaration"
                | "class_declaration"
                | "enum_declaration" => (
                    node.child_by_field_name("name"),
                    view.ancestor(node, TS_SCOPES)?,
                ),
                "import_specifier" => (
                    node.child_by_field_name("alias")
                        .or_else(|| node.child_by_field_name("name")),
                    view.ancestor(node, &["program"])?,
                ),
                _ => continue,
            };
            if let Some(pattern) = pattern {
                for name in ts_pattern(view, pattern)? {
                    if view.text(name) == "arguments" {
                        bindings.declarations.insert(name.id());
                        if let Some(scope) = scope {
                            bindings.add(view, name, scope, scope.start_byte());
                        }
                    }
                }
            }
        }
        Ok(Self { bindings })
    }

    pub fn implicit_arguments(&self, node: Node<'_>, view: &View<'_>) -> Result<bool> {
        if !matches!(node.kind(), "identifier" | "shorthand_property_identifier")
            || view.text(node) != "arguments"
            || self.bindings.declarations.contains(&node.id())
        {
            return Ok(false);
        }
        let mut owner = node;
        while let Some(function) = view.ancestor(owner, TS_FUNCTIONS)? {
            if function.kind() != "arrow_function" {
                return Ok(!self.bindings.shadows(
                    view,
                    "arguments",
                    node.start_byte(),
                    Some(function.byte_range()),
                )?);
            }
            owner = function;
        }
        Ok(false)
    }
}

fn ts_pattern<'a>(view: &View<'a>, pattern: Node<'a>) -> Result<Vec<Node<'a>>> {
    let mut pending = vec![pattern];
    let mut names = Vec::new();
    while let Some(node) = pending.pop() {
        view.charge()?;
        match node.kind() {
            "identifier" | "shorthand_property_identifier_pattern" => names.push(node),
            "pair_pattern" => {
                if let Some(value) = node.child_by_field_name("value") {
                    pending.push(value);
                }
            }
            "assignment_pattern" | "object_assignment_pattern" => {
                if let Some(left) = node.child_by_field_name("left") {
                    pending.push(left);
                }
            }
            "rest_pattern" | "object_pattern" | "array_pattern" => {
                pending.extend(view.children(node)?)
            }
            _ => (),
        }
    }
    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn go_import_literal_decoding_preserves_actual_package_identity() {
        for literal in [
            r#""os""#,
            r#""\x6fs""#,
            r#""\u006fs""#,
            r#""\U0000006fs""#,
            r#""\157s""#,
            "`o\rs`",
        ] {
            assert_eq!(go_string(literal).as_deref(), Some("os"), "{literal}");
        }
        assert_eq!(
            go_string(r#""\a\b\f\n\r\t\v\\\"""#).unwrap(),
            "\x07\x08\x0c\n\r\t\x0b\\\""
        );
        for invalid in [
            "os",
            r#""\q""#,
            r#""\x0""#,
            r#""\uD800""#,
            r#""\U00110000""#,
            r#""\xZZ""#,
        ] {
            assert!(go_string(invalid).is_none(), "{invalid}");
        }
    }
}
