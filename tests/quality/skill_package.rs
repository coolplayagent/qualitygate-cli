use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag, TagEnd};
use serde::Deserialize;
use std::path::Path;

#[derive(Deserialize)]
struct CargoManifest {
    package: CargoPackage,
}

#[derive(Deserialize)]
struct CargoPackage {
    version: String,
}

#[derive(Deserialize)]
struct SkillManifest {
    name: String,
    description: String,
    metadata: SkillMetadata,
}

#[derive(Deserialize)]
struct SkillMetadata {
    version: String,
}

#[derive(Deserialize)]
struct OpenAiManifest {
    interface: SkillInterface,
    policy: SkillPolicy,
}

#[derive(Deserialize)]
struct SkillInterface {
    display_name: String,
    short_description: String,
    default_prompt: String,
}

#[derive(Deserialize)]
struct SkillPolicy {
    allow_implicit_invocation: bool,
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative)).unwrap()
}

fn frontmatter(text: &str) -> Result<&str, &'static str> {
    let mut lines = text.split_inclusive('\n');
    let opening = lines.next().ok_or("missing YAML frontmatter")?;
    if opening != "---\n" && opening != "---\r\n" {
        return Err("SKILL.md must begin with YAML frontmatter");
    }
    let start = opening.len();
    let mut end = start;
    for line in lines {
        if matches!(line, "---\n" | "---\r\n" | "---") {
            return Ok(&text[start..end]);
        }
        end += line.len();
    }
    Err("SKILL.md must close YAML frontmatter")
}

#[test]
fn skill_frontmatter_accepts_lf_and_crlf_without_accepting_malformed_delimiters() {
    for newline in ["\n", "\r\n"] {
        for suffix in ["", newline] {
            let yaml = format!("name: qualitygate-cli{newline}");
            let text = format!("---{newline}{yaml}---{suffix}");
            assert_eq!(frontmatter(&text), Ok(yaml.as_str()));
            let parsed: serde_norway::Value =
                serde_norway::from_str(frontmatter(&text).unwrap()).unwrap();
            assert_eq!(parsed["name"], "qualitygate-cli");
        }
    }
    for malformed in [
        "",
        "---",
        "prefix\n---\nname: skill\n---\n",
        " ---\nname: skill\n---\n",
        "---\rname: skill\r---\r",
        "---\nname: skill\n",
        "---\nname: skill\n---trailing\n",
        "---\r\nname: skill\r\n ---\r\n",
    ] {
        assert!(frontmatter(malformed).is_err(), "accepted {malformed:?}");
    }
}

#[test]
fn skill_package_contract_is_complete_and_matches_the_cli_version() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cargo: CargoManifest = toml::from_str(&read(root, "Cargo.toml")).unwrap();
    let skill_text = read(root, "skills/qualitygate-cli/SKILL.md");
    let skill: SkillManifest = serde_norway::from_str(frontmatter(&skill_text).unwrap()).unwrap();
    let openai: OpenAiManifest =
        serde_norway::from_str(&read(root, "skills/qualitygate-cli/agents/openai.yaml")).unwrap();
    let package_readme = read(root, "skills/qualitygate-cli/README.md");
    let operations = read(root, "skills/qualitygate-cli/references/operations.md");
    let rule_guide = read(root, "skills/qualitygate-cli/references/builtin-rules.md");
    let authoring = read(root, "skills/qualitygate-cli/references/rule-authoring.md");
    let release = read(root, ".github/workflows/release.yml");
    for reference in [
        "file-contracts",
        "diagnostic-ratchets",
        "rule-management",
        "selfcheck",
    ] {
        let path = format!("references/{reference}.md");
        assert!(!read(&root.join("skills/qualitygate-cli"), &path).is_empty());
        assert!(
            skill_text.contains(&path),
            "unreachable Skill reference: {path}"
        );
    }

    assert_eq!(skill.name, "qualitygate-cli");
    assert_eq!(skill.metadata.version, cargo.package.version);
    assert!(skill.description.len() <= 1024);
    assert!(!skill.description.contains('\n'));
    for expected in [
        "implementation",
        "bug-fix",
        "refactor",
        "full snapshot-bound check",
        "recheck the final snapshot",
    ] {
        assert!(
            skill.description.contains(expected),
            "missing code-task discovery term: {expected}"
        );
    }
    assert!(skill_text.contains("untrusted repository checkout"));
    assert!(skill_text.contains("Exit code `0`"));
    assert!(skill_text.contains("linux-aarch64"));
    assert!(skill_text.contains("windows-aarch64"));
    assert!(skill_text.contains("QUALITYGATE_BUILTIN_RULES_DIR"));
    assert!(!skill_text.contains("TODO"));
    assert!(skill_text.contains("references/schemas/project-rule.schema.json"));
    assert!(skill_text.contains("references/decision-protocol.md"));
    for expected in [
        "rules validate candidate.yaml",
        "rules generate --input",
        "qualitygate/rules",
        "not constitute that review",
    ] {
        assert!(
            authoring.contains(expected),
            "missing authoring boundary: {expected}"
        );
    }
    let schema: serde_json::Value = serde_json::from_str(&read(
        root,
        "skills/qualitygate-cli/references/schemas/project-rule.schema.json",
    ))
    .unwrap();
    assert_eq!(
        schema,
        qualitygate::config::rule_schema::document().unwrap()
    );
    assert!(openai.policy.allow_implicit_invocation);
    assert_eq!(openai.interface.display_name, "Qualitygate CLI");
    assert!((25..=64).contains(&openai.interface.short_description.len()));
    assert!(
        openai
            .interface
            .default_prompt
            .starts_with("Use $qualitygate-cli")
    );
    for expected in [
        "unfiltered full check",
        "final snapshot",
        "complete pass",
        "separately required checks",
    ] {
        assert!(
            openai.interface.default_prompt.contains(expected),
            "missing final verification prompt term: {expected}"
        );
    }
    for expected in [
        "--profile full",
        "profile: full",
        "scope: repository",
        "scope: task",
        "plan.pending_delivery_checks",
        "gate.complete: true",
        "gate.decision: pass",
    ] {
        assert!(
            skill_text.contains(expected),
            "missing final code-task gate term: {expected}"
        );
    }
    for expected in [
        "assets/linux-x86_64/qualitygate",
        "assets/linux-aarch64/qualitygate",
        "assets/windows-x86_64/qualitygate.exe",
        "assets/windows-aarch64/qualitygate.exe",
        "references/rules/{core,shared,lang-java,lang-python,lang-typescript,lang-go,lang-c,lang-cpp}/*.yaml",
        "references/schemas/project-rule.schema.json",
        "references/schemas/decision.schema.json",
        "references/schemas/feedback.schema.json",
        "references/decision-protocol.md",
        "references/pilot-evidence.md",
        "assets/pilot/observation-v7.json",
        "assets/pilot/observation-v8.json",
        "assets/pilot/observation-v9.json",
        "assets/pilot/observation-v10.json",
        "ClawHub",
        "workflow_dispatch",
    ] {
        assert!(package_readme.contains(expected), "missing {expected}");
    }
    for expected in [
        "--policy-ref",
        "--trust-store",
        "--evidence-dir",
        "incomplete validation",
        "plan.pending_delivery_checks",
        "gate.complete: true",
        "gate.decision: pass",
    ] {
        assert!(operations.contains(expected), "missing {expected}");
    }
    for expected in [
        "security-sensitive-api",
        "todo-marker",
        "import-boundary",
        "commit-message-convention",
        "test-naming-strict",
        "test-annotation-dependency",
        "no-hardcoded-secrets",
        "no-printf-log",
        "no-unsafe-string",
        "no-test-sleep",
        "no-bare-except",
        "no-os-path",
        "no-print",
        "no-emoji",
        "commit-message-format",
        "python-test-naming",
        "shell-hardcoded-secret",
        "shell-debug-mode",
        "shell-env-dump",
        "shell-weak-crypto",
        "shell-password-echo",
        "shell-sql-injection",
        "shell-exec-terminates",
        "shell-assignment-spaces",
        "shell-comparison-spaces",
        "shell-line-start-operator",
        "shell-stream-merge-position",
        "shell-trap-uncapturable",
        "shell-trap-numeric-signal",
        "shell-trap-double-quotes",
        "shell-tilde-path",
        "shell-temp-file-hardcoded",
        "shell-missing-shebang",
        "shell-commented-dead-code",
        "rust-extern-without-abi",
        "rust-untrusted-dynamic-library-loading",
        "rust-unsafe-block-in-macro-definition",
        "ts-no-eval",
        "ts-no-debugger",
        "ts-no-alert",
        "ts-no-implied-eval",
        "ts-no-new-function",
        "ts-eqeqeq",
        "ts-no-extend-native",
        "ts-no-prototype-builtins",
        "ts-secure-randomness",
        "ts-no-unsafe-postmessage",
        "ts-no-commented-code",
        "ts-no-personal-info-in-comments",
        "go-sql-injection",
        "go-insecure-randomness",
        "go-tls-insecure-skip-verify",
        "go-hardcoded-credentials",
        "go-ssh-insecure-ignore-host-key",
        "go-file-permission-creation",
        "go-panic-in-exported-function",
        "go-cgo-cstring-without-defer-free",
        "go-relative-import-path",
        "go-dot-import",
        "go-sensitive-info-in-log",
        "go-float-loop-counter",
        "forbidden_imports",
        "rules list --format json",
    ] {
        assert!(rule_guide.contains(expected), "missing {expected}");
    }
    for expected in [
        "qualitygate-cli-skill-",
        "assets/linux-x86_64/qualitygate",
        "assets/linux-aarch64/qualitygate",
        "assets/windows-x86_64/qualitygate.exe",
        "assets/windows-aarch64/qualitygate.exe",
        "aarch64-unknown-linux-gnu",
        "aarch64-pc-windows-msvc",
        "0xAA64",
        "cargo llvm-cov",
        "cargo package --locked",
        "CLAWHUB_TOKEN",
        "clawhub publish skills/qualitygate-cli",
        "references/rules/shared/security-sensitive-api.yaml",
        "references/clippy-ratchet.yaml",
        "references/eslint-ratchet.yaml",
        "references/golangci-lint-ratchet.yaml",
        "references/ruff-ratchet.yaml",
        "references/gcc-analyzer-ratchet.yaml",
        "references/clang-static-analyzer-ratchet.yaml",
        "references/checkstyle-ratchet.yaml",
        "references/pmd-ratchet.yaml",
        "references/spotbugs-ratchet.yaml",
        "skill:references/rules/shared/security-sensitive-api.yaml",
        "references/schemas/project-rule.schema.json",
        "references/schemas/decision.schema.json",
        "references/schemas/feedback.schema.json",
        "decision-protocol",
        "references/pilot-evidence.md",
        "assets/pilot/observation-v7.json",
        "assets/pilot/observation-v8.json",
        "assets/pilot/observation-v9.json",
        "assets/pilot/observation-v10.json",
        "qualitygate\" rules schema",
    ] {
        assert!(
            release.contains(expected),
            "release workflow missing {expected}"
        );
    }
    for rule in [
        "core/commit-message-convention",
        "shared/test-naming-strict",
        "shared/test-annotation-dependency",
        "shared/no-hardcoded-secrets",
        "shared/no-printf-log",
        "shared/no-unsafe-string",
        "shared/no-test-sleep",
        "shared/no-bare-except",
        "shared/no-os-path",
        "shared/no-print",
        "shared/no-emoji",
        "core/commit-message-format",
        "shared/python-test-naming",
        "shared/shell-hardcoded-secret",
        "shared/shell-debug-mode",
        "shared/shell-env-dump",
        "shared/shell-weak-crypto",
        "shared/shell-password-echo",
        "shared/shell-sql-injection",
        "shared/shell-exec-terminates",
        "shared/shell-assignment-spaces",
        "shared/shell-comparison-spaces",
        "shared/shell-line-start-operator",
        "shared/shell-stream-merge-position",
        "shared/shell-trap-uncapturable",
        "shared/shell-trap-numeric-signal",
        "shared/shell-trap-double-quotes",
        "shared/shell-tilde-path",
        "shared/shell-temp-file-hardcoded",
        "shared/shell-missing-shebang",
        "shared/shell-commented-dead-code",
        "shared/rust-extern-without-abi",
        "shared/rust-untrusted-dynamic-library-loading",
        "shared/rust-unsafe-block-in-macro-definition",
        "lang-typescript/ts-no-eval",
        "lang-typescript/ts-no-debugger",
        "lang-typescript/ts-no-alert",
        "lang-typescript/ts-no-implied-eval",
        "lang-typescript/ts-no-new-function",
        "lang-typescript/ts-eqeqeq",
        "lang-typescript/ts-no-extend-native",
        "lang-typescript/ts-no-prototype-builtins",
        "lang-typescript/ts-secure-randomness",
        "lang-typescript/ts-no-unsafe-postmessage",
        "lang-typescript/ts-no-commented-code",
        "lang-typescript/ts-no-personal-info-in-comments",
        "lang-go/go-sql-injection",
        "lang-go/go-insecure-randomness",
        "lang-go/go-tls-insecure-skip-verify",
        "lang-go/go-hardcoded-credentials",
        "lang-go/go-ssh-insecure-ignore-host-key",
        "lang-go/go-file-permission-creation",
        "lang-go/go-panic-in-exported-function",
        "lang-go/go-cgo-cstring-without-defer-free",
        "lang-go/go-relative-import-path",
        "lang-go/go-dot-import",
        "lang-go/go-sensitive-info-in-log",
        "lang-go/go-float-loop-counter",
        "lang-python/py-eval-exec",
        "lang-python/py-shell-equals-true",
        "lang-python/py-insecure-randomness",
        "lang-python/py-tls-verify-disabled",
        "lang-python/py-yaml-unsafe-load",
        "lang-python/py-sql-string-format",
        "lang-python/py-hardcoded-credentials",
        "lang-python/py-tempfile-mktemp",
        "lang-python/py-bare-except",
        "lang-python/py-mutable-default-argument",
        "lang-python/py-assert-in-production",
        "lang-python/py-sensitive-info-in-log",
        "lang-python/py-pickle-load",
        "lang-c/c-array-safety",
        "lang-c/c-assertion-discipline",
        "lang-c/c-control-flow",
        "lang-c/c-expression-safety",
        "lang-c/c-file-security",
        "lang-c/c-function-safety",
        "lang-c/c-numeric-literal",
        "lang-cpp/cpp-no-realloc",
        "lang-cpp/cpp-no-alloca",
        "lang-cpp/cpp-no-unsafe-memfunc",
        "lang-cpp/cpp-throw-by-value",
        "lang-cpp/cpp-catch-by-reference",
        "lang-cpp/cpp-no-direct-mutex",
        "lang-cpp/cpp-no-std-move-local-return",
        "lang-cpp/cpp-no-unsafe-rand",
        "lang-cpp/cpp-no-throw-spec",
        "lang-java/java-thread-stop",
        "lang-java/java-thread-yield",
        "lang-java/java-manual-gc",
        "lang-java/java-finalizer",
        "lang-java/java-implicit-charset",
        "lang-java/java-implicit-locale",
        "lang-java/java-insecure-random",
        "lang-java/java-weak-crypto",
        "lang-java/java-empty-catch",
        "lang-java/java-finally-exit",
        "lang-java/java-sql-concat",
        "lang-java/java-runtime-exec",
    ] {
        assert!(release.contains(rule), "release workflow missing {rule}");
        assert!(
            root.join(format!(
                "skills/qualitygate-cli/references/rules/{rule}.yaml"
            ))
            .is_file()
        );
    }
}

#[test]
fn published_skill_has_only_bundled_document_references_and_template() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let skill = root.join("skills/qualitygate-cli").canonicalize().unwrap();
    let bundled = read(&skill, "assets/pilot/observation-v7.json");
    assert_eq!(bundled, read(root, "templates/pilot/observation-v7.json"));
    let template: serde_json::Value = serde_json::from_str(&bundled).unwrap();
    assert_eq!(template["schema_version"], 7);
    let bundled_v8 = read(&skill, "assets/pilot/observation-v8.json");
    assert_eq!(
        bundled_v8,
        read(root, "templates/pilot/observation-v8.json")
    );
    let template_v8: serde_json::Value = serde_json::from_str(&bundled_v8).unwrap();
    assert_eq!(template_v8["schema_version"], 8);
    let bundled_v9 = read(&skill, "assets/pilot/observation-v9.json");
    assert_eq!(
        bundled_v9,
        read(root, "templates/pilot/observation-v9.json")
    );
    let template_v9: serde_json::Value = serde_json::from_str(&bundled_v9).unwrap();
    assert_eq!(template_v9["schema_version"], 9);
    let bundled_v10 = read(&skill, "assets/pilot/observation-v10.json");
    assert_eq!(
        bundled_v10,
        read(root, "templates/pilot/observation-v10.json")
    );
    let template_v10: serde_json::Value = serde_json::from_str(&bundled_v10).unwrap();
    assert_eq!(template_v10["schema_version"], 10);
    assert!(template_v10["protocol"]["budget"].is_null());
    assert!(
        template_v10["protocol"]["thresholds"]
            .get("cost_ratio_max")
            .is_none()
    );

    for file in super::files(&skill) {
        if file.extension().is_none_or(|extension| extension != "md") {
            continue;
        }
        let markdown = std::fs::read_to_string(&file).unwrap();
        for unavailable in ["docs/pilot-phase-", "templates/pilot/", "../../.github/"] {
            assert!(
                !markdown.contains(unavailable),
                "{} references an unpackaged repository path: {unavailable}",
                file.display()
            );
        }
        for event in Parser::new(&markdown) {
            if let Event::Start(Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. }) = event {
                let destination = dest_url.split('#').next().unwrap();
                if destination.is_empty() {
                    continue;
                }
                assert!(
                    !destination.contains("://"),
                    "{} links outside the Skill package: {destination}",
                    file.display()
                );
                let resolved = file
                    .parent()
                    .unwrap()
                    .join(destination)
                    .canonicalize()
                    .unwrap();
                assert!(
                    resolved.starts_with(&skill) && resolved.is_file(),
                    "{} links outside the Skill package: {destination}",
                    file.display()
                );
            }
        }
    }
}

#[test]
fn skill_capability_examples_match_runtime_validation() {
    fn example(name: &str) -> serde_json::Value {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let text = read(
            root,
            &format!("skills/qualitygate-cli/references/{name}.md"),
        );
        let mut examples = Vec::new();
        let mut yaml = None;
        for event in Parser::new(&text) {
            match event {
                Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(kind)))
                    if kind.as_ref() == "yaml" =>
                {
                    yaml = Some(String::new());
                }
                Event::Text(text) if yaml.is_some() => yaml.as_mut().unwrap().push_str(&text),
                Event::End(TagEnd::CodeBlock) => {
                    if let Some(text) = yaml.take() {
                        examples.push(text);
                    }
                }
                _ => {}
            }
        }
        assert_eq!(
            examples.len(),
            1,
            "expected one complete YAML example in {name}"
        );
        serde_norway::from_str(&examples[0]).unwrap()
    }

    let mut rule = example("file-contracts");
    rule["source"]["content_hash"] = qualitygate::snapshot::digest(b"fixture policy").into();
    assert!(qualitygate::config::rule_schema::parse(&serde_json::to_vec(&rule).unwrap()).is_ok());
    rule["when"]["change"] = "any".into();
    assert!(qualitygate::config::rule_schema::parse(&serde_json::to_vec(&rule).unwrap()).is_err());

    let mut config = example("diagnostic-ratchets");
    assert!(qualitygate::config::parse(&serde_json::to_vec(&config).unwrap()).is_ok());
    config["checks"][0]["reports"][0]
        .as_object_mut()
        .unwrap()
        .remove("baseline");
    assert!(qualitygate::config::parse(&serde_json::to_vec(&config).unwrap()).is_err());
}

#[test]
fn executable_catalog_reads_skill_reference_rules_without_compiled_manifests() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for expected in [
        "core/line-ending.yaml",
        "shared/security-sensitive-api.yaml",
        "shared/todo-marker.yaml",
        "shared/import-boundary.yaml",
        "lang-java/module-boundary.yaml",
        "lang-python/pytest-naming.yaml",
        "lang-c/c-array-safety.yaml",
        "lang-cpp/cpp-no-realloc.yaml",
        "lang-java/java-thread-stop.yaml",
    ] {
        assert!(
            root.join("skills/qualitygate-cli/references/rules")
                .join(expected)
                .is_file(),
            "missing Skill rule asset: {expected}"
        );
    }
    assert!(!root.join("skills/qualitygate-cli/rules").exists());
    let config = qualitygate::config::Config {
        rulesets: qualitygate::config::RULESETS
            .iter()
            .map(|package| (*package).into())
            .collect(),
        ..qualitygate::config::Config::default()
    };
    let catalog = qualitygate::config::catalog::Catalog::load(&config, std::iter::empty()).unwrap();
    assert!(catalog.entries.len() >= 14);
    assert!(catalog.entries.values().all(|entry| {
        entry.builtin.is_some() && entry.origin.starts_with("skill:references/rules/")
    }));
    let catalog = read(root, "src/qualitygate/config/catalog.rs");
    let environment = read(root, "src/qualitygate/env/mod.rs");
    assert!(!catalog.contains("include_str!(\"../../../qualitygate/rules/"));
    assert!(environment.contains("BUILTIN_RULES_DIR_ENV"));
}
