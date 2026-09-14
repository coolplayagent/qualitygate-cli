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
    assert!(skill_text.contains("untrusted repository checkout"));
    assert!(skill_text.contains("Exit code `0`"));
    assert!(skill_text.contains("linux-aarch64"));
    assert!(skill_text.contains("windows-aarch64"));
    assert!(skill_text.contains("QUALITYGATE_BUILTIN_RULES_DIR"));
    assert!(!skill_text.contains("TODO"));
    assert!(skill_text.contains("references/schemas/project-rule.schema.json"));
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
        "assets/linux-x86_64/qualitygate",
        "assets/linux-aarch64/qualitygate",
        "assets/windows-x86_64/qualitygate.exe",
        "assets/windows-aarch64/qualitygate.exe",
        "references/rules/{core,shared,lang-java,lang-python}/*.yaml",
        "references/schemas/project-rule.schema.json",
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
    ] {
        assert!(operations.contains(expected), "missing {expected}");
    }
    for expected in [
        "security-sensitive-api",
        "todo-marker",
        "import-boundary",
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
        "skill:references/rules/shared/security-sensitive-api.yaml",
        "references/schemas/project-rule.schema.json",
        "qualitygate\" rules schema",
    ] {
        assert!(
            release.contains(expected),
            "release workflow missing {expected}"
        );
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
