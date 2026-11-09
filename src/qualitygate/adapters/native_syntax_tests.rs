use super::*;

fn matches(id: &str, source: &str) -> usize {
    let deadline = Instant::now() + Duration::from_secs(2);
    let tree = parse(source.as_bytes(), "cpp", deadline).unwrap();
    let mut pending = vec![tree.root_node()];
    let mut count = 0;
    while let Some(node) = pending.pop() {
        count += usize::from(matches_rule(id, node, source.as_bytes(), deadline, &mut 0).unwrap());
        let mut cursor = node.walk();
        pending.extend(node.children(&mut cursor));
    }
    count
}

#[test]
fn syntax_distinguishes_tokens_and_calls_from_documentation_and_unrelated_names() {
    for (id, source) in [
        ("c-no-atoi", "int f() { return std::atoi(text); }"),
        ("c-no-atoi", "int f() { return ::atoll(text); }"),
        ("c-no-atoi", "int f() { return ::std::atoi(text); }"),
        (
            "c-no-process-launch",
            "void f() { ::CreateProcessW(args); }",
        ),
        ("c-no-goto", "void f() { goto\n end; end: return; }"),
        ("cpp-no-const-cast", "auto x = const_cast\n<int*>(p);"),
        (
            "cpp-no-reinterpret-cast",
            "auto x = reinterpret_cast /* rationale */ <int*>(p);",
        ),
        (
            "cpp-no-delete-this",
            "void f() { delete /* owned */ (/* me */ this); }",
        ),
        ("cpp-use-nullptr", "auto x = NULL;"),
        (
            "cpp-no-using-namespace-in-header",
            "namespace x { using\n namespace std; }",
        ),
    ] {
        assert_eq!(matches(id, source), 1, "{id}: {source}");
        let documentation = format!("/* {source} */\nconst char *s = R\"x({source})x\";");
        // Comments containing nested comment delimiters are not valid C++; use the literal alone.
        let documentation = if source.contains("/*") {
            format!("const char *s = R\"x({source})x\";")
        } else {
            documentation
        };
        assert_eq!(matches(id, &documentation), 0, "{id}: documentation");
    }
    for (id, source) in [
        (
            "c-no-atoi",
            "void f() { custom::atoi(x); obj.atoi(x); obj->atof(x); }",
        ),
        (
            "c-no-process-launch",
            "int system(const char*); void f() { custom::system(x); obj.system(x); }",
        ),
        ("cpp-no-const-cast", "auto x = static_cast<int>(p);"),
        ("cpp-no-delete-this", "void f() { delete other; }"),
        ("cpp-use-nullptr", "auto x = nullptr; int NULLABLE = 1;"),
        (
            "cpp-no-using-namespace-in-header",
            "using std::string; namespace x = std;",
        ),
    ] {
        assert_eq!(matches(id, source), 0, "{id}: {source}");
    }
}

#[test]
fn smart_pointer_rule_recognizes_constructors_without_matching_other_allocations() {
    for source in [
        "std::unique_ptr<Widget> x{new Widget()};",
        "::std::unique_ptr<Widget> x{new Widget()};",
        "auto x = std::shared_ptr<Widget>{new Widget()};",
        "std::unique_ptr<Widget> x(new Widget());",
        "auto x = std::shared_ptr<Widget>(new Widget());",
        "std::unique_ptr<std::vector<int>> x(new std::vector<int>());",
    ] {
        assert_eq!(
            matches("cpp-prefer-make-smart-pointer", source),
            1,
            "{source}"
        );
    }
    for source in [
        "void f() { auto x = new Widget(); }",
        "auto x = std::make_shared<Widget>();",
        "auto x = custom::unique_ptr<Widget>(new Widget());",
        "auto x = unique_ptr<Widget>(new Widget());",
        "auto x = std::wrapper<Widget>(new Widget());",
        "std::unique_ptr<Widget> x; auto other = new Widget();",
        "auto x = std::unique_ptr<Widget>(factory(new Widget()));",
        "auto x = std::unique_ptr<Widget>{factory()};",
    ] {
        assert_eq!(
            matches("cpp-prefer-make-smart-pointer", source),
            0,
            "{source}"
        );
    }
}

#[test]
fn numeric_macros_require_a_single_literal_expression() {
    for value in [
        "16",
        "-16",
        "(+16)",
        "0xffU",
        "0b1010",
        "1.5e-2",
        "16 /* count */",
        "16 // count",
    ] {
        assert!(
            numeric_macro(value, Instant::now() + Duration::from_secs(2), &mut 0).unwrap(),
            "{value}"
        );
        assert_eq!(
            matches("cpp-no-macro-constant", &format!("#define LIMIT {value}\n")),
            1,
            "{value}"
        );
    }
    for value in [
        "",
        "identifier",
        "1 + 2",
        "\"16\"",
        "sizeof(int)",
        "1; f()",
        "1, other = 2",
        "~1",
        "(",
    ] {
        assert!(
            !numeric_macro(value, Instant::now() + Duration::from_secs(2), &mut 0).unwrap(),
            "{value}"
        );
    }
    assert_eq!(
        matches("cpp-no-macro-constant", "#define FLAG\n#define F(x) 16\n"),
        0
    );
}

#[test]
fn native_parsing_fails_closed_on_invalid_or_unbounded_input() {
    let deadline = Instant::now() + Duration::from_secs(2);
    assert!(parse(b"void f( {", "cpp", deadline).is_err());
    assert!(parse(&vec![b' '; MAX_BYTES + 1], "c", deadline).is_err());
    assert!(parse(b"int x;", "c", Instant::now()).is_err());
    assert!(numeric_macro("16", Instant::now(), &mut 0).is_err());
    assert!(parse(b"int x;", "c", deadline).is_ok());
}

fn snapshot(path: &str, bytes: Vec<u8>, added_lines: &[usize]) -> Snapshot {
    use crate::snapshot::{Change, File, Identity};
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
                bytes,
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
                added_lines: added_lines.iter().copied().collect(),
            },
        )]
        .into(),
        path_filter: None,
        commits: Vec::new(),
    }
}

fn evaluate(
    id: &str,
    snapshot: &Snapshot,
    parameters: serde_json::Value,
) -> crate::domain::CheckResult {
    super::super::rules::evaluate_as(
        id,
        "native-syntax",
        &RuleSetting {
            parameters: serde_json::from_value(parameters).unwrap(),
            ..RuleSetting::default()
        },
        snapshot,
    )
}

#[test]
fn evaluator_rejects_missing_invalid_and_unbounded_evidence() {
    let config = serde_json::json!({"languages":["cpp"]});
    for source in [
        vec![255],
        b"void f( {".to_vec(),
        vec![b' '; MAX_BYTES + 1],
        "int x;\n".repeat(50_000).into_bytes(),
    ] {
        let result = evaluate(
            "cpp-use-nullptr",
            &snapshot("x.cpp", source, &[1]),
            config.clone(),
        );
        assert!(result.verdict.is_none(), "{:?}", result.execution);
    }
    let mut input = snapshot("x.cpp", b"auto x = NULL;".to_vec(), &[1]);
    input.files.clear();
    assert!(
        evaluate("cpp-use-nullptr", &input, config.clone())
            .verdict
            .is_none()
    );
    input.changes.get_mut("x.cpp").unwrap().kind = "deleted".into();
    assert_eq!(
        evaluate("cpp-use-nullptr", &input, config.clone()).verdict,
        Some(crate::domain::Verdict::Pass)
    );
    let input = snapshot("x.cpp", b"auto x = NULL;".to_vec(), &[1]);
    for parameters in [
        serde_json::json!({}),
        serde_json::json!({"languages":["c"]}),
    ] {
        assert!(
            evaluate("cpp-use-nullptr", &input, parameters)
                .verdict
                .is_none()
        );
    }
    assert!(evaluate("unknown", &input, config).verdict.is_none());
}

#[test]
fn evaluator_selects_fixed_languages_paths_and_changed_ranges() {
    let source = b"void f() {\ngoto end; end: return;\n}\n".to_vec();
    for path in ["x.c", "x.h", "x.cpp"] {
        let input = snapshot(path, source.clone(), &[2]);
        let result = evaluate(
            "c-no-goto",
            &input,
            serde_json::json!({"languages":["c","cpp"]}),
        );
        assert_eq!(result.diagnostics.len(), 1, "{path}");
    }
    for (path, lines, config) in [
        ("x.c", vec![2], serde_json::json!({"languages":["cpp"]})),
        ("x.txt", vec![2], serde_json::json!({"languages":["cpp"]})),
        ("x.cpp", vec![], serde_json::json!({"languages":["cpp"]})),
        (
            "x.cpp",
            vec![2],
            serde_json::json!({"languages":["cpp"],"paths":["include/**"]}),
        ),
    ] {
        let result = evaluate(
            "cpp-use-nullptr",
            &snapshot(path, source.clone(), &lines),
            config,
        );
        assert_eq!(result.verdict, Some(crate::domain::Verdict::Pass));
    }
    let input = snapshot("x.cpp", b"using namespace std;".to_vec(), &[1]);
    assert!(
        evaluate(
            "cpp-no-using-namespace-in-header",
            &input,
            serde_json::json!({"languages":["cpp"]})
        )
        .diagnostics
        .is_empty()
    );
}

#[test]
fn numeric_macro_subtrees_share_the_file_node_budget() {
    let mut visited = MAX_NODES;
    assert!(numeric_macro("16", Instant::now() + Duration::from_secs(2), &mut visited).is_err());
}

#[test]
fn c_family_rules_accept_both_c_and_cpp_header_syntax() {
    for source in [
        "struct C { int value; }; void f() { goto end; end: return; }",
        "namespace api { class C { void f() { goto end; end: return; } }; }",
        "int f(int x) { int n = _Generic(x, int: 1, default: 2); goto end; end: return n; }",
    ] {
        let result = evaluate(
            "c-no-goto",
            &snapshot("api.h", source.as_bytes().to_vec(), &[1]),
            serde_json::json!({"languages":["c","cpp"]}),
        );
        assert_eq!(
            result.diagnostics.len(),
            1,
            "{source}: {:?}",
            result.execution
        );
    }
}
