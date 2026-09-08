use super::*;

pub(super) fn extract(language: &str, path: &str, node: Node<'_>, bytes: &[u8]) -> Option<Entity> {
    let mut annotations = BTreeMap::new();
    let name = match (language, node.kind()) {
        ("java", "method_declaration") => {
            annotations = java_annotations(node, bytes);
            if !annotations.keys().any(|name| {
                [
                    "Test",
                    "ParameterizedTest",
                    "RepeatedTest",
                    "TestFactory",
                    "TestTemplate",
                ]
                .contains(&name.as_str())
            }) {
                return None;
            }
            text(node.child_by_field_name("name")?, bytes).to_string()
        }
        ("python", "function_definition") => {
            if let Some(parent) = node
                .parent()
                .filter(|parent| parent.kind() == "decorated_definition")
            {
                let mut cursor = parent.walk();
                for decorator in parent
                    .named_children(&mut cursor)
                    .filter(|node| node.kind() == "decorator")
                {
                    let raw = text(decorator, bytes).trim_start_matches('@');
                    let name = raw
                        .split('(')
                        .next()
                        .unwrap_or(raw)
                        .rsplit('.')
                        .next()
                        .unwrap_or(raw);
                    annotations.insert(name.into(), raw.into());
                }
            }
            let name = text(node.child_by_field_name("name")?, bytes);
            if !name.starts_with("test_") {
                return None;
            }
            name.into()
        }
        ("rust", "function_item") => {
            let mut previous = node.prev_named_sibling();
            while let Some(attribute) = previous.filter(|node| node.kind() == "attribute_item") {
                let raw = text(attribute, bytes)
                    .trim_start_matches("#[")
                    .trim_end_matches(']');
                let name = raw
                    .split('(')
                    .next()
                    .unwrap_or(raw)
                    .rsplit("::")
                    .next()
                    .unwrap_or(raw);
                annotations.insert(name.into(), raw.into());
                previous = attribute.prev_named_sibling();
            }
            if !annotations
                .keys()
                .any(|key| ["test", "rstest", "test_case"].contains(&key.as_str()))
            {
                return None;
            }
            text(node.child_by_field_name("name")?, bytes).into()
        }
        ("go", "function_declaration") => {
            let name = text(node.child_by_field_name("name")?, bytes);
            let parameters = text(node.child_by_field_name("parameters")?, bytes);
            if !name.starts_with("Test")
                || name == "TestMain"
                || name.chars().nth(4).is_some_and(char::is_lowercase)
                || !parameters.contains(".T")
            {
                return None;
            }
            name.into()
        }
        ("typescript", "call_expression") => {
            let function = node.child_by_field_name("function")?;
            let callee = text(function, bytes);
            if !(callee == "it"
                || callee == "test"
                || callee.starts_with("it.")
                || callee.starts_with("test."))
            {
                return None;
            }
            if !(path.contains(".test.")
                || path.contains(".spec.")
                || path.contains("__tests__")
                || bytes.windows(6).any(|window| window == b"vitest")
                || bytes.windows(11).any(|window| window == b"@jest/globa"))
            {
                return None;
            }
            let arguments = node.child_by_field_name("arguments")?;
            let first = arguments.named_child(0)?;
            if !matches!(first.kind(), "string" | "template_string") {
                return None;
            }
            if callee.contains("each") {
                annotations.insert("parameterized".into(), callee.into());
            }
            text(first, bytes).trim_matches(['\'', '"', '`']).into()
        }
        _ => return None,
    };
    let body = node.child_by_field_name("body").unwrap_or(node);
    let mut normalized = String::new();
    let mut shape = String::new();
    let mut pending = vec![body];
    while let Some(child) = pending.pop() {
        if child.kind().contains("comment") {
            continue;
        }
        shape.push_str(child.kind());
        shape.push(';');
        if child.child_count() == 0 {
            normalized.push_str(text(child, bytes));
            normalized.push('\0');
        } else {
            let mut cursor = child.walk();
            let children: Vec<_> = child.children(&mut cursor).collect();
            pending.extend(children.into_iter().rev());
        }
    }
    let owner = owner(node, bytes);
    let signature = if language == "java" {
        node.child_by_field_name("parameters")
            .map(|parameters| {
                let mut cursor = parameters.walk();
                let types: Vec<_> = parameters
                    .named_children(&mut cursor)
                    .map(|parameter| {
                        parameter
                            .child_by_field_name("type")
                            .map(|value| text(value, bytes))
                            .unwrap_or_else(|| text(parameter, bytes))
                            .split_whitespace()
                            .collect::<String>()
                    })
                    .collect();
                format!("({})", types.join(","))
            })
            .unwrap_or_default()
    } else {
        String::new()
    };
    let mut entity_range = range(node);
    let mut start_byte = node.start_byte();
    if language == "python"
        && let Some(parent) = node
            .parent()
            .filter(|parent| parent.kind() == "decorated_definition")
    {
        entity_range.start_line = range(parent).start_line;
        start_byte = parent.start_byte();
    }
    if language == "rust" {
        let mut previous = node.prev_named_sibling();
        while let Some(attribute) = previous.filter(|node| node.kind() == "attribute_item") {
            entity_range.start_line = range(attribute).start_line;
            start_byte = attribute.start_byte();
            previous = attribute.prev_named_sibling();
        }
    }
    Some(Entity {
        symbol: format!("{owner}#{name}{signature}"),
        name,
        range: entity_range,
        byte_range: start_byte..node.end_byte(),
        annotations,
        body_digest: crate::snapshot::digest(normalized.as_bytes()),
        shape_digest: crate::snapshot::digest(shape.as_bytes()),
    })
}

fn java_annotations(node: Node<'_>, bytes: &[u8]) -> BTreeMap<String, String> {
    let mut values = BTreeMap::new();
    let mut cursor = node.walk();
    for modifiers in node
        .named_children(&mut cursor)
        .filter(|node| node.kind() == "modifiers")
    {
        let mut cursor = modifiers.walk();
        for annotation in modifiers.named_children(&mut cursor) {
            if !["annotation", "marker_annotation"].contains(&annotation.kind()) {
                continue;
            }
            if let Some(name) = annotation.child_by_field_name("name") {
                values.insert(
                    text(name, bytes)
                        .rsplit('.')
                        .next()
                        .unwrap_or_default()
                        .into(),
                    text(annotation, bytes).into(),
                );
            }
        }
    }
    values
}

fn owner(mut node: Node<'_>, bytes: &[u8]) -> String {
    let mut owners = Vec::new();
    while let Some(parent) = node.parent() {
        if [
            "class_declaration",
            "class_definition",
            "mod_item",
            "impl_item",
        ]
        .contains(&parent.kind())
            && let Some(name) = parent
                .child_by_field_name("name")
                .or_else(|| parent.child_by_field_name("type"))
        {
            owners.push(text(name, bytes));
        }
        node = parent;
    }
    owners.reverse();
    owners.join("::")
}
