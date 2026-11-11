//! Literal Shell command/argument classification; expansions are never executed.
use crate::source_analysis::syntax::review::View;
use anyhow::Result;
use tree_sitter::Node;

pub(crate) fn matches(id: &str, node: Node<'_>, view: &View<'_>) -> Result<bool> {
    if id == "shell-comparison-spaces" {
        return Ok(node.kind() == "test_command" && missing_test_spaces(node, view));
    }
    if node.kind() != "command" {
        return Ok(false);
    }
    let Some(name) = node.child_by_field_name("name") else {
        return Ok(false);
    };
    let mut command_name = name;
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
        command_name = *name;
        offset += 1;
    }
    let args = &args[offset..];
    match (id, command.as_str()) {
        ("shell-assignment-spaces", _) => Ok(split_assignment(command_name, args, view)),
        ("shell-trap-uncapturable", "trap") => uncapturable_trap(args, view),
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

fn split_assignment(name: Node<'_>, args: &[Node<'_>], view: &View<'_>) -> bool {
    let raw_name = view.text(name).as_bytes();
    let identifier = raw_name
        .first()
        .is_some_and(|byte| byte.is_ascii_alphabetic() || *byte == b'_')
        && raw_name
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'_');
    let Some(argument) = args.first() else {
        return false;
    };
    let text = view.text(*argument);
    identifier
        && name.end_byte() < argument.start_byte()
        && matches!(argument.kind(), "word" | "concatenation")
        && (text.starts_with('=') || text.starts_with("+="))
}

fn missing_test_spaces(node: Node<'_>, view: &View<'_>) -> bool {
    let text = view.text(node).as_bytes();
    let width = if text.starts_with(b"[[") { 2 } else { 1 };
    text.get(width)
        .is_some_and(|byte| !byte.is_ascii_whitespace())
        || text
            .len()
            .checked_sub(width + 1)
            .and_then(|index| text.get(index))
            .is_some_and(|byte| !byte.is_ascii_whitespace())
}

fn uncapturable_trap(args: &[Node<'_>], view: &View<'_>) -> Result<bool> {
    let mut index = 0;
    let Some(first) = args.first() else {
        return Ok(false);
    };
    let Some(mut action) = literal(*first, view)? else {
        return Ok(false);
    };
    if action == "--" {
        index += 1;
        let Some(argument) = args.get(index) else {
            return Ok(false);
        };
        let Some(value) = literal(*argument, view)? else {
            return Ok(false);
        };
        action = value;
    } else if action.starts_with('-') {
        // -p/-P/-l query traps, and '-' restores default handling.
        return Ok(false);
    }
    if action == "-" || action.chars().all(|ch| ch.is_ascii_digit()) && !action.is_empty() {
        // With a numeric first operand Bash omits the action and resets signals.
        return Ok(false);
    }
    for argument in &args[index + 1..] {
        let Some(signal) = literal(*argument, view)? else {
            continue;
        };
        if matches!(
            signal.to_ascii_uppercase().as_str(),
            "KILL" | "SIGKILL" | "STOP" | "SIGSTOP" | "9"
        ) {
            return Ok(true);
        }
    }
    Ok(false)
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

pub(crate) fn literal(node: Node<'_>, view: &View<'_>) -> Result<Option<String>> {
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
            "word" | "string_content" | "number" => {
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
