//! Literal Shell command/argument classification; expansions are never executed.
use super::syntax::review::View;
use anyhow::Result;
use tree_sitter::Node;

pub(super) fn matches(id: &str, node: Node<'_>, view: &View<'_>) -> Result<bool> {
    if node.kind() != "command" {
        return Ok(false);
    }
    let Some(name) = node.child_by_field_name("name") else {
        return Ok(false);
    };
    let Some(mut command) = literal(name, view)? else {
        return Ok(false);
    };
    let mut cursor = node.walk();
    let mut args = Vec::new();
    for argument in node.children_by_field_name("argument", &mut cursor) {
        view.charge()?;
        args.push(argument);
    }
    let mut offset = 0;
    // These builtins explicitly select another command; -v/-V only query it.
    if matches!(command.as_str(), "command" | "builtin") {
        while let Some(arg) = args.get(offset) {
            let Some(word) = literal(*arg, view)? else {
                return Ok(false);
            };
            offset += 1;
            if word == "--" {
                break;
            }
            if command == "command" && word == "-p" {
                continue;
            }
            if word.starts_with('-') {
                return Ok(false);
            }
            offset -= 1;
            break;
        }
        let Some(name) = args.get(offset) else {
            return Ok(false);
        };
        let Some(word) = literal(*name, view)? else {
            return Ok(false);
        };
        command = word;
        offset += 1;
    }
    let args = &args[offset..];
    match (id, command.as_str()) {
        ("shell-eval", "eval") => Ok(true),
        ("shell-alias", "alias") => {
            for arg in args {
                view.charge()?;
                if let Some(word) = literal(*arg, view)?
                    && word
                        .split_once('=')
                        .is_some_and(|(name, _)| !name.is_empty() && !name.starts_with('-'))
                {
                    return Ok(true);
                }
                // A static name= prefix also defines an alias with an expanded value.
                if arg.kind() == "concatenation"
                    && let Some(first) = view.children(*arg)?.first()
                    && let Some(prefix) = literal(*first, view)?
                    && prefix
                        .split_once('=')
                        .is_some_and(|(name, _)| !name.is_empty() && !name.starts_with('-'))
                {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        ("shell-chmod-read", "chmod" | "/bin/chmod" | "/usr/bin/chmod") => {
            let mut options = true;
            for arg in args {
                let Some(word) = literal(*arg, view)? else {
                    return Ok(false);
                };
                if options && word == "--" {
                    options = false;
                    continue;
                }
                if options
                    && matches!(
                        word.as_str(),
                        "--recursive"
                            | "--changes"
                            | "--silent"
                            | "--quiet"
                            | "--verbose"
                            | "--preserve-root"
                            | "--no-preserve-root"
                    )
                {
                    continue;
                }
                if options && word.starts_with("--reference") {
                    return Ok(false);
                }
                if options && word.starts_with('-') {
                    if word.len() > 1 && word[1..].chars().all(|ch| "Rcfv".contains(ch)) {
                        continue;
                    }
                    return Ok(false);
                }
                return Ok(removes_all_read(&word));
            }
            Ok(false)
        }
        _ => Ok(false),
    }
}

fn removes_all_read(mode: &str) -> bool {
    let mut found = false;
    for clause in mode.split(',') {
        let Some(index) = clause.find(['+', '-', '=']) else {
            return false;
        };
        let (who, operation) = clause.split_at(index);
        if !who.chars().all(|ch| "augo".contains(ch)) {
            return false;
        }
        // Multiple operations in a clause are supported, without claiming final permissions.
        let mut operations = operation.chars().peekable();
        while let Some(op) = operations.next() {
            if !"+-=".contains(op) {
                return false;
            }
            let mut read = false;
            while operations.peek().is_some_and(|ch| !"+-=".contains(*ch)) {
                let ch = operations.next().expect("peeked permission");
                if !"rwxXstugo".contains(ch) {
                    return false;
                }
                read |= ch == 'r';
            }
            found |= op == '-'
                && read
                && (who.contains('a') || "ugo".chars().all(|ch| who.contains(ch)));
        }
    }
    found
}

pub(super) fn literal(node: Node<'_>, view: &View<'_>) -> Result<Option<String>> {
    // A tiny quote decoder over AST-selected literal forms, not a source regex.
    let mut stack = vec![node];
    let mut result = String::new();
    while let Some(node) = stack.pop() {
        view.charge()?;
        match node.kind() {
            "command_name" | "concatenation" | "string" => {
                let mut children = view.children(node)?;
                children.reverse();
                stack.extend(children);
            }
            "raw_string" => {
                let text = view.text(node);
                result.push_str(&text[1..text.len() - 1]);
            }
            "word" | "string_content" => {
                let quoted = node.kind() == "string_content";
                let mut chars = view.text(node).chars().peekable();
                while let Some(ch) = chars.next() {
                    if ch == '\\' {
                        if let Some(next) = chars.next() {
                            if next == '\n' {
                                continue;
                            }
                            if quoted && !"$`\"\\".contains(next) {
                                result.push('\\');
                            }
                            result.push(next);
                        } else {
                            return Ok(None);
                        }
                    } else {
                        result.push(ch);
                    }
                }
            }
            _ => return Ok(None),
        }
    }
    Ok(Some(result))
}
