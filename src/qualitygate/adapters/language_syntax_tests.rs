use super::*;
use crate::{
    domain::{CheckResult, Verdict},
    snapshot::{Change, File, Identity},
};

fn snapshot(path: &str, source: &[u8], lines: &[usize]) -> Snapshot {
    Snapshot {
        scope_evidence: Default::default(),
        root: ".".into(),
        identity: Identity {
            verification_digest: None,
            merge_request: None,
            mode: "worktree".into(),
            base: "base".into(),
            head: "head".into(),
            content_digest: "digest".into(),
        },
        files: [(
            path.into(),
            File {
                bytes: source.to_vec(),
                executable: false,
            },
        )]
        .into(),
        base_files: Default::default(),
        changes: [(
            path.into(),
            Change {
                kind: "modified".into(),
                old_path: Some(path.into()),
                removed_lines: Default::default(),
                added_lines: lines.iter().copied().collect(),
            },
        )]
        .into(),
        path_filter: None,
        commits: Vec::new(),
    }
}
fn evaluate(id: &str, input: &Snapshot) -> CheckResult {
    super::super::rules::evaluate_as(
        id,
        "language-syntax",
        &RuleSetting {
            parameters: [(
                "languages".into(),
                serde_json::json!([language(id).unwrap()]),
            )]
            .into(),
            ..RuleSetting::default()
        },
        input,
    )
}
fn check(id: &str, path: &str, source: &str, count: usize) {
    let lines: Vec<_> = (1..=source.lines().count()).collect();
    let result = evaluate(id, &snapshot(path, source.as_bytes(), &lines));
    assert!(
        result.verdict.is_some(),
        "{id}: {source}: {:?}",
        result.execution
    );
    assert_eq!(result.diagnostics.len(), count, "{id}: {source}");
}

#[test]
fn python_rules_use_expression_shapes_including_interpolations() {
    for (id, source, count) in [
        (
            "py-none-comparison",
            "if (None) == value != other:\n    pass\n",
            1,
        ),
        (
            "py-none-comparison",
            "if None is value == other:\n    pass\n",
            0,
        ),
        (
            "py-none-comparison",
            "if value != (\n    None\n):\n    pass\n",
            1,
        ),
        (
            "py-none-comparison",
            "text = 'x == None'\n# None != value\n",
            0,
        ),
        ("py-none-comparison", "text = f'{value == None}'\n", 1),
        (
            "py-walrus-operator",
            "if (count := len(items)):\n    pass\n",
            1,
        ),
        ("py-walrus-operator", "text = f'{(count := 1)}'\n", 1),
        ("py-walrus-operator", "text = ':='\n# count := 1\n", 0),
        (
            "py-lambda-assignment",
            "handler: Callable = (\n    lambda x: x + 1\n)\n",
            1,
        ),
        ("py-lambda-assignment", "handler = lambda x: x\n", 1),
        (
            "py-lambda-assignment",
            "run(lambda x: x)\nobj.handler = lambda x: x\n",
            0,
        ),
        (
            "py-lambda-assignment",
            "text = '''\nhandler = lambda x: x\n'''\n",
            0,
        ),
    ] {
        check(id, "sample.py", source, count);
    }
}

#[test]
fn go_imports_and_exit_calls_are_bound_to_actual_packages() {
    for (id, source, count) in [
        (
            "go-import-alias",
            "package p\nimport ( f \"fmt\"; j \"encoding/json\" )\n",
            2,
        ),
        (
            "go-import-alias",
            "package p\nimport (_ \"fmt\"; . \"os\")\n",
            0,
        ),
        (
            "go-os-exit",
            "package p\nimport stop \"os\"\nfunc f(){ stop.Exit(1) }\n",
            1,
        ),
        (
            "go-os-exit",
            "package p\nimport . \"runtime\"\nfunc f(){ Goexit() }\n",
            1,
        ),
        (
            "go-os-exit",
            "package p\nimport . \"os\"\nfunc f(Exit func(int)){ Exit(1) }\n",
            0,
        ),
        (
            "go-os-exit",
            "package p\nimport os \"example.com/os\"\nfunc f(){ os.Exit(1) }\n",
            0,
        ),
        ("go-os-exit", "package p\nfunc f(){ os.Exit(1) }\n", 0),
        (
            "go-os-exit",
            "package p\nimport \"os\"\nfunc f(os Service){ os.Exit(1) }\n",
            0,
        ),
        (
            "go-os-exit",
            "package p\nimport \"os\"\nfunc f(){ os := factory(); os.Exit(1) }\n",
            0,
        ),
        (
            "go-os-exit",
            "package p\nimport \"os\"\nfunc f(){ if os := factory(); true { os.Exit(1) } }\n",
            0,
        ),
        (
            "go-os-exit",
            "package p\nimport \"os\"\nfunc f(){ for os := range items { os.Exit(1) } }\n",
            0,
        ),
        (
            "go-os-exit",
            "package p\nimport \"os\"\nfunc f(){ { os := factory(); os.Exit(1) }; os.Exit(2) }\n",
            1,
        ),
        (
            "go-os-exit",
            "package p\nimport o \"o\\x73\"\nfunc f(){ o.Exit(1) }\n",
            1,
        ),
        (
            "go-unsafe-package",
            "package p\nimport u `unsafe`\nvar pointer u.Pointer\n",
            1,
        ),
        (
            "go-unsafe-package",
            "package p\nimport . \"unsafe\"\nvar pointer Pointer\n",
            1,
        ),
        (
            "go-unsafe-package",
            "package p\nvar example = `unsafe.Pointer(x)`\n",
            0,
        ),
    ] {
        check(id, "sample.go", source, count);
    }
}

#[test]
fn go_retained_imports_and_changed_bindings_preserve_review_scope() {
    for (id, source, changed, count) in [
        (
            "go-unsafe-package",
            "package p\nimport u \"unsafe\"\nvar ptr u.Pointer\n",
            vec![3],
            1,
        ),
        (
            "go-unsafe-package",
            "package p\nimport . \"unsafe\"\nvar ptr Pointer\n",
            vec![3],
            1,
        ),
        (
            "go-unsafe-package",
            "package p\nimport u \"unsafe\"\nfunc f(u T){ u.Pointer(x) }\n",
            vec![3],
            0,
        ),
        (
            "go-os-exit",
            "package p\nimport stop \"os\"\nfunc f(){ stop.Exit(1) }\n",
            vec![3],
            1,
        ),
        (
            "go-os-exit",
            "package p\nimport stop \"os\"\nfunc f(){ stop.Exit(1) }\n",
            vec![2],
            1,
        ),
        (
            "go-os-exit",
            "package p\nimport stop \"os\"\n// comment only\nfunc f(){ stop.Exit(1) }\n",
            vec![3],
            0,
        ),
    ] {
        let result = evaluate(id, &snapshot("sample.go", source.as_bytes(), &changed));
        assert!(result.verdict.is_some(), "{:?}", result.execution);
        assert_eq!(result.diagnostics.len(), count, "{source}");
        if changed == [2] {
            assert_eq!(result.diagnostics[0].range.as_ref().unwrap().start_line, 2);
        }
    }
}

#[test]
fn typescript_identifies_declarations_and_implicit_arguments_not_properties_or_bindings() {
    for (id, source, count) in [
        ("ts-no-var", "var x = 1; const obj = {var: 'var x'};", 1),
        ("ts-no-var", "for (var x of xs) { run(x); }", 1),
        ("ts-no-var", "const text = `var x`; let x = 1;", 0),
        (
            "ts-no-var",
            "const text = `${(() => { var x = 1; return x; })()}`;",
            1,
        ),
        ("ts-no-with", "function f(){ with\n(obj) { run(); } }", 1),
        ("ts-no-with", "const obj = {with: fn}; obj.with();", 0),
        (
            "ts-no-arguments",
            "function f(){ return arguments.length; }",
            1,
        ),
        (
            "ts-no-arguments",
            "function f(){ return (() => arguments[0])(); }",
            1,
        ),
        ("ts-no-arguments", "function f(){ return {arguments}; }", 1),
        (
            "ts-no-arguments",
            "function f(){ return `${arguments[0]}`; }",
            1,
        ),
        (
            "ts-no-arguments",
            "function f(arguments){ return arguments.length; }",
            0,
        ),
        (
            "ts-no-arguments",
            "function f(...arguments){ return arguments.length; }",
            0,
        ),
        (
            "ts-no-arguments",
            "function f({arguments}){ return arguments; }",
            0,
        ),
        (
            "ts-no-arguments",
            "function f(){ const arguments = xs; return arguments; }",
            0,
        ),
        (
            "ts-no-arguments",
            "function f(){ return (arguments => arguments)(1); }",
            0,
        ),
        (
            "ts-no-arguments",
            "function f(){ for (const arguments of xs) { run(arguments); } }",
            0,
        ),
        (
            "ts-no-arguments",
            "function f(){ try {} catch(arguments){ run(arguments); } }",
            0,
        ),
        (
            "ts-no-arguments",
            "function f(){ return obj.arguments; }",
            0,
        ),
        (
            "ts-no-arguments",
            "function f(){ return {arguments: 1}; }",
            0,
        ),
        (
            "ts-no-arguments",
            "const f = function arguments() { return 1; };",
            0,
        ),
        (
            "ts-no-arguments",
            "function f(arguments){ return function g(){ return arguments; }; }",
            1,
        ),
        (
            "ts-no-arguments",
            "function f(){ { let arguments = 1; run(arguments); } return arguments; }",
            1,
        ),
        (
            "ts-no-arguments",
            "const text = 'arguments'; const obj = { arguments: 1 }; obj.arguments;",
            0,
        ),
    ] {
        check(id, "sample.ts", source, count);
    }
    check(
        "ts-no-arguments",
        "sample.tsx",
        "function F(){return <div>{arguments[0]}</div>;}",
        1,
    );
    check(
        "ts-no-arguments",
        "sample.jsx",
        "function F(){return <div>arguments</div>;}",
        0,
    );
}

#[test]
fn shell_rules_distinguish_executed_commands_from_data_and_mode_from_paths() {
    for (id, source, count) in [
        ("shell-eval", "eval \"$cmd\"\n", 1),
        ("shell-eval", "e'va'l \"$cmd\"\n", 1),
        ("shell-eval", "command -- eval x\n", 1),
        ("shell-eval", "builtin eval x\n", 1),
        ("shell-eval", "command -v eval\n", 0),
        ("shell-eval", "echo eval\n# eval x\n", 0),
        ("shell-eval", "echo \"$(eval x)\"\n", 1),
        ("shell-eval", "cat <<'EOF'\neval x\nEOF\n", 0),
        ("shell-eval", "cat <<EOF\n$(eval x)\nEOF\n", 1),
        ("shell-eval", "\"$command\" x\n", 0),
        ("shell-alias", "alias ll='ls -l'\n", 1),
        ("shell-alias", "alias ll=\"$cmd\"\n", 1),
        ("shell-alias", "alias ll\nalias -p\n", 0),
        ("shell-chmod-read", "chmod -R -- 'a-r' file\n", 1),
        ("shell-chmod-read", "chmod --recursive ugo-rwx file\n", 1),
        ("shell-chmod-read", "chmod a-rw,u+x file\n", 1),
        ("shell-chmod-read", "chmod 0644 ./a-r\n", 0),
        ("shell-chmod-read", "echo chmod a-r\n", 0),
        ("shell-chmod-read", "chmod --reference=example ./a-r\n", 0),
        ("shell-chmod-read", "chmod -- -R a-r\n", 0),
        ("shell-chmod-read", "chmod u-r file\n", 0),
        ("shell-chmod-read", "chmod \"$mode\" a-r\n", 0),
    ] {
        check(id, "sample.sh", source, count);
    }
    check(
        "shell-eval",
        "scripts/run",
        "#!/usr/bin/env -S bash -e\neval x\n",
        1,
    );
}

#[test]
fn comment_only_changes_do_not_re_report_retained_syntax() {
    for (id, path, source, lines) in [
        (
            "py-none-comparison",
            "x.py",
            "if value == (\n# rationale\nNone):\n    pass\n",
            vec![2],
        ),
        ("ts-no-with", "x.ts", "with (obj) {\nrun();\n}\n", vec![2]),
        ("shell-eval", "x.sh", "eval x\n# rationale\n", vec![2]),
    ] {
        assert_eq!(
            evaluate(id, &snapshot(path, source.as_bytes(), &lines)).verdict,
            Some(Verdict::Pass)
        );
    }
}

#[test]
fn incomplete_evidence_never_becomes_a_successful_empty_result() {
    for source in [
        vec![255],
        b"if (".to_vec(),
        vec![b' '; review::MAX_BYTES + 1],
        "x = 1\n".repeat(50_000).into_bytes(),
    ] {
        assert!(
            evaluate("py-none-comparison", &snapshot("x.py", &source, &[1]))
                .verdict
                .is_none()
        );
    }
    let mut input = snapshot("x.py", b"value == None\n", &[1]);
    input.files.clear();
    assert!(evaluate("py-none-comparison", &input).verdict.is_none());
    input.changes.get_mut("x.py").unwrap().kind = "deleted".into();
    assert_eq!(
        evaluate("py-none-comparison", &input).verdict,
        Some(Verdict::Pass)
    );
    assert!(review::parse(b"x=1\n", "python", "x.py", Instant::now()).is_err());
    assert!(
        review::parse(
            b"x=1\n",
            "unknown",
            "x.py",
            Instant::now() + Duration::from_secs(2)
        )
        .is_err()
    );
}

#[test]
fn type_only_parameters_and_unrelated_qualified_types_do_not_change_runtime_bindings() {
    check(
        "ts-no-arguments",
        "sample.ts",
        "function f(){ type Fn = (arguments: number) => void; return arguments; }",
        1,
    );
    check(
        "go-os-exit",
        "sample.go",
        "package p\nimport \"os\"\nfunc f(v any) { switch os := v.(type) { default: os.Exit(1) } }\n",
        0,
    );
    let source = "package p\nimport . \"unsafe\"\nvar p other.Pointer\nvar v = T{Pointer: x}\n";
    let result = evaluate(
        "go-unsafe-package",
        &snapshot("sample.go", source.as_bytes(), &[3, 4]),
    );
    assert_eq!(result.verdict, Some(Verdict::Pass));
    check(
        "go-os-exit",
        "sample.go",
        "package p\nimport . \"os\"\nimport . \"runtime\"\nfunc f(){ Exit(1); Goexit() }\n",
        2,
    );
}

#[test]
fn go_import_review_respects_parameter_type_switch_and_range_shadowing() {
    for source in [
        "package p\nimport \"os\"\nfunc f(os interface{ Exit(int) }) { os.Exit(1) }\n",
        "package p\nimport \"os\"\nfunc f(value interface{}) { switch os := value.(type) { case interface{ Exit(int) }: os.Exit(1) } }\n",
        "package p\nimport \"os\"\nfunc f(values []interface{ Exit(int) }) { for _, os := range values { os.Exit(1) } }\n",
        "package p\nimport _ \"os\"\nfunc f() {}\n",
    ] {
        check("go-os-exit", "main.go", source, 0);
    }
    check(
        "go-os-exit",
        "main.go",
        r#"package p
import "\u006fs"
func f() { os.Exit(1) }
"#,
        1,
    );
}

#[test]
fn implicit_arguments_respects_destructured_bindings_and_arrow_lexical_ownership() {
    for source in [
        "function f({value: arguments}) { return arguments; }",
        "function f([arguments = []]) { return arguments; }",
        "function f(...arguments) { return arguments; }",
        "const f = arguments => arguments;",
        "import { value as arguments } from 'pkg'; const f = () => arguments;",
        "function f() { for (var arguments of list) { use(arguments); } }",
    ] {
        check("ts-no-arguments", "main.ts", source, 0);
    }
}

#[test]
fn shell_wrappers_quoting_and_permission_operations_preserve_literal_command_meaning() {
    for (id, source, count) in [
        ("shell-eval", "command -p -- eval 'echo unsafe'", 1),
        ("shell-eval", "builtin -- eval 'echo unsafe'", 1),
        ("shell-eval", "command -v eval", 0),
        ("shell-eval", "command --", 0),
        ("shell-eval", "command -- \"$cmd\"", 0),
        ("shell-eval", "command \"$flag\" eval", 0),
        ("shell-eval", "\"e\\qval\" true", 0),
        ("shell-alias", "alias name=\"$value\"", 1),
        ("shell-alias", "alias \"$name\"=value", 0),
        (
            "shell-chmod-read",
            "chmod --recursive --verbose -- ugo-r file",
            1,
        ),
        ("shell-chmod-read", "chmod -Rcfv a+r-w-r file", 1),
        ("shell-chmod-read", "chmod --reference=other file", 0),
        ("shell-chmod-read", "chmod -Z a-r file", 0),
        ("shell-chmod-read", "chmod \"$mode\" file", 0),
        ("shell-chmod-read", "chmod --recursive", 0),
        ("shell-chmod-read", "chmod 000 file", 0),
        ("shell-chmod-read", "chmod z-r file", 0),
        ("shell-chmod-read", "chmod a-q file", 0),
        ("shell-chmod-read", "chmod a-r,u+x file", 1),
    ] {
        check(id, "script.sh", source, count);
    }
}

#[test]
fn sql_formatting_distinguishes_python_operators_from_database_placeholders() {
    for (source, count) in [
        (r#"cursor.execute("SELECT id WHERE id=%s", (user_id,))"#, 0),
        (
            r#"cursor.executemany("SELECT id WHERE id=%(id)s", rows)"#,
            0,
        ),
        (r#"cursor.execute("SELECT id WHERE id=?", (user_id,))"#, 0),
        (r#"cursor.execute("SELECT id WHERE id=%s" % user_id)"#, 1),
        (r#"cursor.execute("SELECT id WHERE id=%s" % (user_id,))"#, 1),
        ("execute(query % user_id)", 1),
        (
            r#"cursor.execute("SELECT id WHERE id={}".format(user_id))"#,
            1,
        ),
        (r#"cursor.execute(f"SELECT id WHERE id={user_id}")"#, 1),
        ("cursor.execute(\n    (query % user_id)\n)\n", 1),
        (r#"cursor.execute("SELECT id", (value % 2,))"#, 0),
        (r#"cursor.execute(".format(x) % value")"#, 0),
        (r#"cursor.execute(f"SELECT id")"#, 0),
        ("cursor.execute()", 0),
        ("cursor.execute(query=query)", 0),
        ("other(query % value)", 0),
        ("# cursor.execute(query % user_id)\n", 0),
    ] {
        check("py-sql-string-format", "query.py", source, count);
    }
}

#[test]
fn shell_brackets_are_checked_only_in_test_commands() {
    for (source, count) in [
        (r#"["$flag" = yes ]"#, 1),
        (r#"[ "$flag" = yes]"#, 1),
        (r#"[ "$flag" = "$other"]"#, 1),
        (r#"[ "$flag" = yes ]"#, 0),
        ("[[$a == $b]]", 1),
        ("[[ $a == $b]]", 1),
        ("[[foo == bar ]]", 1),
        ("if [[ $a == $b]]; then :; fi", 1),
        (r"[[ $a == value\\]]", 1),
        (r"[[ $a == value\\\\]]", 1),
        (r#"[[ "$flag" = "]]" ]]"#, 0),
        ("items=(one two)\nprintf '%s' \"${items[0]}\"\n", 0),
        ("printf '%s' [ab]*\n", 0),
        (r#"printf '%s' '[[$a == $b]]'"#, 0),
        ("# [[$a == $b]]\n", 0),
    ] {
        check("shell-comparison-spaces", "script.sh", source, count);
    }
}

#[test]
fn shell_assignment_review_uses_command_position_instead_of_physical_lines() {
    for (source, count) in [
        ("VALUE = bad", 1),
        ("VALUE =bad", 1),
        ("VALUE += bad", 1),
        ("VALUE =\"$data\"", 1),
        ("command VALUE = bad", 1),
        ("if ready; then VALUE = bad; fi", 1),
        ("IFS= read -r line", 0),
        ("VALUE= command", 0),
        ("VALUE=", 0),
        ("local VALUE = bad", 0),
        ("[ value = other ]", 0),
        ("printf '%s' \"VALUE = text\"", 0),
        ("printf '%s' '\nVALUE = text\n'", 0),
        ("cat <<'EOF'\nVALUE = text\nEOF\n", 0),
        ("printf '%s' \\\n# VALUE = text\n", 0),
        ("VALUE '=literal'", 0),
        ("\"VALUE\" = literal", 0),
        ("./VALUE = literal", 0),
        // Command intent is unresolved: this shape also matches valid commands.
        ("echo = bad", 1),
    ] {
        check("shell-assignment-spaces", "script.sh", source, count);
    }
}

#[test]
fn shell_traps_check_signal_operands_instead_of_action_text() {
    for (source, count) in [
        ("trap 'printf 9' EXIT", 0),
        (r#"trap 'kill -9 "$child"' EXIT"#, 0),
        ("trap cleanup KILL", 1),
        ("trap cleanup STOP", 1),
        ("trap cleanup SIGKILL", 1),
        ("trap cleanup SIGSTOP", 1),
        ("trap cleanup kill", 1),
        ("trap cleanup sigstop", 1),
        ("trap cleanup 9", 1),
        ("trap cleanup 19", 0),
        ("trap cleanup 17 23 24", 0),
        ("trap '' KILL", 1),
        ("trap -- cleanup 'KILL'", 1),
        ("command -p -- trap cleanup SIGSTOP", 1),
        ("builtin trap cleanup KILL", 1),
        ("trap 'cleanup' \\\n  STOP", 1),
        ("trap -p KILL", 0),
        ("trap -P KILL", 0),
        ("trap -l", 0),
        ("trap - KILL", 0),
        ("trap -- - KILL", 0),
        ("trap 9", 0),
        ("trap KILL", 0),
        ("trap", 0),
        ("trap --", 0),
        ("trap -- \"$handler\" KILL", 0),
        ("trap \"$handler\" KILL", 0),
        ("trap cleanup \"$signal\"", 0),
        ("trap cleanup \"$signal\" KILL", 1),
        ("printf '%s' 'trap cleanup KILL'", 0),
        ("# trap cleanup KILL\n", 0),
    ] {
        check("shell-trap-uncapturable", "script.sh", source, count);
    }
}

#[test]
fn bounded_comparison_recovery_cannot_hide_other_syntax_errors() {
    for source in [
        "[[$a == $b]]\nif (\n",
        "[[$a == $b]]\n[ $x = $y\n",
        "[[ $a == $b\n",
        "[[ $a == 'quoted]]'\n",
        "[[ $a == \"quoted]]\"\n",
        "[[ $a == value\\]\\]\n",
        r"[[ $a == value\]]",
        r"[[ $a == value\\\]]",
    ] {
        let result = evaluate(
            "shell-comparison-spaces",
            &snapshot("x.sh", source.as_bytes(), &[1, 2]),
        );
        assert!(result.verdict.is_none(), "{source}: {:?}", result.execution);
    }
    for (id, path, invalid) in [
        ("py-sql-string-format", "x.py", "cursor.execute("),
        ("shell-trap-uncapturable", "x.sh", "trap 'unterminated"),
        ("shell-assignment-spaces", "x.sh", "VALUE 'unterminated"),
        ("shell-comparison-spaces", "x.sh", "if ("),
    ] {
        for bytes in [invalid.as_bytes(), &[255]] {
            assert!(evaluate(id, &snapshot(path, bytes, &[1])).verdict.is_none());
        }
    }
    assert!(review::parse_shell_comparisons(b"[[$a == $b]]", "x.sh", Instant::now()).is_err());
    // The recovery is unavailable to other Shell syntax rules.
    assert!(
        evaluate(
            "shell-trap-uncapturable",
            &snapshot("x.sh", b"[[$a == $b]]", &[1])
        )
        .verdict
        .is_none()
    );
    let repeated = "[[$a == $b]]\n".repeat(6_000);
    assert!(
        evaluate(
            "shell-comparison-spaces",
            &snapshot("x.sh", repeated.as_bytes(), &[1])
        )
        .verdict
        .is_none(),
        "Recovery and matching must share the 200000-step budget"
    );
}

#[test]
fn frozen_v1_review_definitions_keep_their_original_source_pattern_behavior() {
    use crate::config::{
        Config,
        catalog::{Builtin, Catalog, Entry},
    };
    for (id, language, path, pattern, source) in [
        (
            "py-sql-string-format",
            "python",
            "x.py",
            r"\b(?:execute|executemany)\s*\([^)]*%\s*\w+",
            r#"cursor.execute("SELECT %s", (value,))"#,
        ),
        (
            "shell-comparison-spaces",
            "shell",
            "x.sh",
            r"(?:\[\[[^\s\[]|\[[^\s\[]|[^\s\]]\]\]?)",
            "printf '%s' [ab]*",
        ),
        (
            "shell-trap-uncapturable",
            "shell",
            "x.sh",
            r"(?i)\btrap\b.*\b(?:SIGKILL|9|SIGSTOP|19)\b",
            "trap 'printf 9' EXIT",
        ),
        (
            "shell-assignment-spaces",
            "shell",
            "x.sh",
            r"^\s*[A-Za-z_][A-Za-z0-9_]*(?:\s+\+?=|\+?=\s+)",
            "IFS= read -r line",
        ),
    ] {
        let definition: Builtin = serde_json::from_value(serde_json::json!({
            "id":id,"version":1,"description":"Frozen definition","implementation":"source-pattern",
            "language":[language],"requires_capabilities":["files"],"defaults":{"required":true,"severity":"error",
            "parameters":{"languages":[language],"prohibited_patterns":{language:[pattern]}}}
        })).unwrap();
        let catalog = Catalog {
            entries: [(
                id.into(),
                Entry {
                    origin: "frozen".into(),
                    package: "shared".into(),
                    builtin: Some(definition),
                    custom: None,
                },
            )]
            .into(),
        };
        let original = serde_json::to_vec(&catalog).unwrap();
        let restored: Catalog = serde_json::from_slice(&original).unwrap();
        let config = restored
            .resolve(&Config {
                rules: [(id.into(), RuleSetting::default())].into(),
                ..Config::default()
            })
            .unwrap();
        let builtin = restored.entries[id].builtin.as_ref().unwrap();
        let result = super::super::rules::evaluate_as(
            id,
            &builtin.implementation,
            &config.rules[id],
            &snapshot(path, source.as_bytes(), &[1]),
        );
        assert_eq!(result.verdict, Some(Verdict::Fail), "{id}");
        assert_eq!(serde_json::to_vec(&restored).unwrap(), original);
        assert_eq!(builtin.version, 1);
    }
}
