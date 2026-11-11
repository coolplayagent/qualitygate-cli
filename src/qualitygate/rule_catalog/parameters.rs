//! Discoverable parameter contracts shared by CLI edits and policy validation.

use crate::domain::rule_parameters::{self, LanguageContract, PatternLanguages};
use anyhow::{Result, bail};
use serde_json::{Value, json};

pub(crate) fn names(implementation: &str) -> Result<&'static [&'static str]> {
    Ok(match implementation {
        "line-ending" => &[],
        "commit-message" => &["pattern"],
        "diff-size" => &["max_added_lines"],
        "test-naming" => &["pattern", "patterns", "paths", "languages"],
        "test-naming-strict" => &["pattern", "paths", "languages"],
        "test-annotation-dependency" => &["annotation", "group", "artifact", "paths", "languages"],
        "file-pattern" => &["pattern", "paths", "languages"],
        "language-syntax" | "native-syntax" | "shell-shebang" | "shell-commented-code" => {
            &["paths", "languages"]
        }
        "parameterized-tests" => &["minimum_similar", "paths", "languages"],
        "comment-language" => &["language", "exempt_patterns", "paths", "languages"],
        "ai-code-traceability" => &["marker", "provenance_scope", "paths", "languages"],
        "source-pattern" => &["prohibited_patterns", "paths", "languages"],
        "import-boundary" => &["forbidden_imports", "paths", "languages"],
        "used-undeclared" => &["modules"],
        "module-boundary" => &["modules", "forbidden", "dependency_kind"],
        _ => bail!("Unknown builtin implementation: {implementation}"),
    })
}

pub fn describe(implementation: &str, defaults: &Value) -> Result<Vec<Value>> {
    let declared: Vec<String> = defaults
        .get("languages")
        .map(|languages| serde_json::from_value(languages.clone()))
        .transpose()?
        .unwrap_or_default();
    let contract = LanguageContract::builtin(implementation, &declared, None)?;
    describe_contract(implementation, defaults, &contract)
}

pub fn describe_builtin(builtin: &crate::rule_catalog::catalog::Builtin) -> Result<Vec<Value>> {
    let contract = LanguageContract::builtin(
        &builtin.implementation,
        &builtin.language,
        builtin.effective_language_scope(),
    )?;
    describe_contract(
        &builtin.implementation,
        &serde_json::to_value(&builtin.defaults.parameters)?,
        &contract,
    )
}

fn describe_contract(
    implementation: &str,
    defaults: &Value,
    contract: &LanguageContract<'_>,
) -> Result<Vec<Value>> {
    names(implementation)?.iter().map(|name| {
        let (kind, description, schema) = match *name {
            "pattern" => ("string", "Regular expression matched against the selected commit subject, test name or source line.", json!({"type":"string","minLength":1,"maxLength":512})),
            "annotation" => ("string", "Simple Java annotation name selecting test methods.", json!({"type":"string","minLength":1,"maxLength":128,"pattern":"^[A-Za-z_$][A-Za-z0-9_$]*$"})),
            "group" | "artifact" => ("string", "Required Maven dependency coordinate component.", json!({"type":"string","minLength":1,"maxLength":128,"pattern":"^[A-Za-z0-9_.-]+$"})),
            "patterns" => ("object", "Language names mapped to test-name regular expressions.", json!({"type":"object","additionalProperties":{"type":"string"}})),
            "prohibited_patterns" => ("object", "Declared language scope mapped to 1..32 distinct regular expressions, each 1..512 UTF-8 bytes; semantic validation enforces the byte limit and regex syntax.", contract.pattern_schema()),
            "forbidden_imports" => ("object", "Supported syntax languages (or all) mapped to 1..32 distinct regular expressions, each 1..512 UTF-8 bytes; semantic validation enforces the byte limit and regex syntax.", rule_parameters::pattern_schema(PatternLanguages::Syntax)),
            "paths" => ("array", "Repository-relative glob patterns selecting source files.", strings()),
            "languages" => ("array", "Language scope from this built-in's definition; fixed lists and selectable subsets are expressed by the schema.", contract.language_schema()),
            "exempt_patterns" => ("array", "Regular expressions exempting matching comments.", strings()),
            "modules" => ("array", "1..256 unique normalized repository module roots.", json!({"type":"array","minItems":1,"maxItems":256,"uniqueItems":true,"items":{"type":"string"}})),
            "max_added_lines" => ("integer", "Maximum added lines in the selected diff (at least one).", json!({"type":"integer","minimum":1})),
            "minimum_similar" => ("integer", "Minimum similar tests before requiring parameterization (at least two).", json!({"type":"integer","minimum":2})),
            "language" => ("string", "Required comment language.", json!({"type":"string","enum":["chinese","english","bilingual"]})),
            "provenance_scope" => ("string", "Whether all additions or only verified agent-participating additions require markers.", json!({"type":"string","enum":["all_added_tests","ai_only"]})),
            "dependency_kind" => ("string", "Whether module directions use declared or resolved dependencies.", json!({"type":"string","enum":["declared","resolved"]})),
            "marker" => ("object", "Required annotation, comment, or Git trailer marker with name and optional field names.", json!({"type":"object","required":["type","name"],"additionalProperties":false,"properties":{"type":{"enum":["annotation","comment","git_trailer"]},"name":{"type":"string","minLength":1},"fields":strings()}})),
            "forbidden" => ("array", "1..256 forbidden Maven dependency directions; from/to are group:artifact globs, scopes are optional.", json!({"type":"array","minItems":1,"maxItems":256,"items":{"type":"object","required":["from","to"],"additionalProperties":false,"properties":{"from":{"type":"string"},"to":{"type":"string"},"scopes":strings()}}})),
            _ => unreachable!("parameter inventory is closed"),
        };
        let mut descriptor = json!({"name":name,"type":kind,"description":description,"default":defaults[name],"has_default":defaults.get(name).is_some(),"schema":schema});
        if *name == "languages" {
            descriptor["required"] = json!(contract.requires_languages());
        }
        Ok(descriptor)
    }).collect()
}

fn strings() -> Value {
    json!({"type":"array","items":{"type":"string"}})
}

/// Values are JSON; dotted paths edit one object member while retaining siblings.
pub(crate) fn apply(parameters: &mut Value, edits: &[String], implementation: &str) -> Result<()> {
    if edits.len() > 128 {
        bail!("At most 128 parameter edits are allowed");
    }
    if edits.iter().map(String::len).sum::<usize>() > crate::config::MAX_CONFIG_BYTES {
        bail!("Parameter edits exceed 1 MiB");
    }
    let mut paths = Vec::<String>::new();
    let names = names(implementation)?;
    for edit in edits {
        if edit.len() > crate::config::MAX_CONFIG_BYTES {
            bail!("Parameter edit exceeds 1 MiB");
        }
        let (key, value) = edit
            .split_once('=')
            .ok_or_else(|| anyhow::anyhow!("Expected --param key=JSON"))?;
        let parts: Vec<_> = key.split('.').collect();
        if parts.len() > 16
            || parts.iter().any(|part| part.is_empty())
            || !names.contains(&parts[0])
        {
            bail!("Unsupported {implementation} parameter: {key}");
        }
        if paths.iter().any(|previous| {
            previous == key
                || previous.starts_with(&format!("{key}."))
                || key.starts_with(&format!("{previous}."))
        }) {
            bail!("Repeated or overlapping parameter path: {key}");
        }
        let value: Value = serde_json::from_str(value)?;
        let mut target = &mut *parameters;
        for part in &parts[..parts.len() - 1] {
            target = target
                .as_object_mut()
                .ok_or_else(|| anyhow::anyhow!("Parameter path must address an object: {key}"))?
                .entry(*part)
                .or_insert_with(|| json!({}));
        }
        target
            .as_object_mut()
            .ok_or_else(|| anyhow::anyhow!("Parameter path must address an object: {key}"))?
            .insert(parts[parts.len() - 1].into(), value);
        paths.push(key.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shipped_source_rule_schemas_and_validation_share_language_contracts() {
        let config = crate::config::Config {
            rulesets: crate::config::RULESETS
                .iter()
                .map(|name| (*name).into())
                .collect(),
            ..Default::default()
        };
        let catalog = crate::rule_catalog::catalog::Catalog::load(&config, []).unwrap();
        for entry in catalog.entries.values() {
            let builtin = entry.builtin.as_ref().unwrap();
            if builtin.implementation != "source-pattern" && builtin.id != "python-test-naming" {
                continue;
            }
            let descriptors = describe_builtin(builtin).unwrap();
            let mut setting = builtin.defaults.clone();
            if !builtin.language.is_empty() {
                setting
                    .parameters
                    .entry("languages".into())
                    .or_insert_with(|| json!(builtin.language));
            }
            for descriptor in &descriptors {
                let name = descriptor["name"].as_str().unwrap();
                if let Some(value) = setting.parameters.get(name) {
                    let schema = jsonschema::draft202012::new(&descriptor["schema"]).unwrap();
                    assert!(schema.is_valid(value), "{}.{name}: {value}", builtin.id);
                }
            }
            let schema = &descriptors
                .iter()
                .find(|item| item["name"] == "languages")
                .unwrap()["schema"];
            let validator = jsonschema::draft202012::new(schema).unwrap();
            for candidate in [
                json!([]),
                json!(["java"]),
                json!(["rust"]),
                json!(["c", "cpp"]),
                json!(["cpp", "c"]),
                json!(["unknown"]),
                json!([true]),
            ] {
                let mut candidate_setting = setting.clone();
                candidate_setting
                    .parameters
                    .insert("languages".into(), candidate.clone());
                let accepted = crate::rule_catalog::builtin_validation::validate_with_scope(
                    &builtin.id,
                    &builtin.implementation,
                    &builtin.language,
                    builtin.language_scope,
                    &candidate_setting,
                )
                .is_ok();
                assert_eq!(
                    validator.is_valid(&candidate),
                    accepted,
                    "{} languages {candidate}",
                    builtin.id
                );
            }
            // Identity is provenance, not an implicit source of scope semantics.
            let mut renamed = builtin.clone();
            renamed.id = "review-renamed".into();
            assert_eq!(describe_builtin(&renamed).unwrap(), descriptors);
            crate::rule_catalog::builtin_validation::validate_with_scope(
                &renamed.id,
                &renamed.implementation,
                &renamed.language,
                renamed.language_scope,
                &setting,
            )
            .unwrap();
        }
    }

    #[test]
    fn dotted_edits_preserve_siblings_and_reject_ambiguous_or_unbounded_updates() {
        let original = json!({"patterns":{"rust":"^test_","python":"^test_"}});
        let mut changed = original.clone();
        apply(
            &mut changed,
            &[r#"patterns.rust="^scenario_""#.into()],
            "test-naming",
        )
        .unwrap();
        assert_eq!(changed["patterns"]["rust"], "^scenario_");
        assert_eq!(changed["patterns"]["python"], "^test_");
        for edits in [
            vec!["patterns.rust=1".into(), "patterns={}".into()],
            vec!["patterns={}".into(), "patterns.rust=1".into()],
            vec!["paths=[]".into(), "paths=[]".into()],
            vec!["unknown=true".into()],
            vec!["patterns..rust=1".into()],
            vec!["pattern".into()],
            vec!["paths=not-json".into()],
            vec!["paths=[]".into(); 129],
            vec![format!("{}=1", "patterns.".repeat(17))],
            vec![format!(
                "paths=\"{}\"",
                "x".repeat(crate::config::MAX_CONFIG_BYTES)
            )],
        ] {
            assert!(apply(&mut original.clone(), &edits, "test-naming").is_err());
        }
        assert!(
            apply(
                &mut json!({"patterns":[]}),
                &["patterns.rust=1".into()],
                "test-naming"
            )
            .is_err()
        );
        assert!(apply(&mut json!([]), &["paths=[]".into()], "test-naming").is_err());
        let mut created = json!({});
        apply(
            &mut created,
            &["patterns.rust=\"test\"".into()],
            "test-naming",
        )
        .unwrap();
        assert_eq!(created, json!({"patterns":{"rust":"test"}}));
    }
    #[test]
    fn discoverable_contracts_expose_types_defaults_and_fixed_scopes() {
        for implementation in [
            "line-ending",
            "commit-message",
            "diff-size",
            "test-naming",
            "test-naming-strict",
            "test-annotation-dependency",
            "file-pattern",
            "language-syntax",
            "native-syntax",
            "shell-shebang",
            "shell-commented-code",
            "parameterized-tests",
            "comment-language",
            "ai-code-traceability",
            "source-pattern",
            "import-boundary",
            "used-undeclared",
            "module-boundary",
        ] {
            let descriptions = describe(
                implementation,
                &json!({"languages":["java"],"max_added_lines":42}),
            )
            .unwrap();
            assert_eq!(descriptions.len(), names(implementation).unwrap().len());
            for item in descriptions {
                assert!(item["description"].as_str().unwrap().len() > 10);
                assert!(item["schema"].is_object());
            }
        }
        assert!(describe("unknown", &json!({})).is_err());
        let sizes = describe("diff-size", &json!({"max_added_lines":42})).unwrap();
        assert_eq!(sizes[0]["default"], 42);
        assert_eq!(sizes[0]["has_default"], true);
    }
}
