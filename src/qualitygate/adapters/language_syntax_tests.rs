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
