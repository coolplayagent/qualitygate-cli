//! Discoverable parameter contracts shared by CLI edits and policy validation.

use anyhow::{Result, bail};
use serde_json::{Value, json};

pub(super) fn names(implementation: &str) -> Result<&'static [&'static str]> {
    Ok(match implementation {
        "line-ending" => &[],
        "commit-message" => &["pattern"],
        "diff-size" => &["max_added_lines"],
        "test-naming" => &["pattern", "patterns", "paths", "languages"],
        "test-naming-strict" => &["pattern", "paths", "languages"],
        "test-annotation-dependency" => &["annotation", "group", "artifact", "paths", "languages"],
        "file-pattern" => &["pattern", "paths", "languages"],
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
    names(implementation)?.iter().map(|name| {
        let (kind, description, schema) = match *name {
            "pattern" => ("string", "Regular expression matched against the selected commit subject, test name or source line.", json!({"type":"string","minLength":1,"maxLength":512})),
            "annotation" => ("string", "Simple Java annotation name selecting test methods.", json!({"type":"string","minLength":1,"maxLength":128,"pattern":"^[A-Za-z_$][A-Za-z0-9_$]*$"})),
            "group" | "artifact" => ("string", "Required Maven dependency coordinate component.", json!({"type":"string","minLength":1,"maxLength":128,"pattern":"^[A-Za-z0-9_.-]+$"})),
            "patterns" => ("object", "Language names mapped to test-name regular expressions.", json!({"type":"object","additionalProperties":{"type":"string"}})),
            "prohibited_patterns" | "forbidden_imports" => ("object", "Supported language names (or all) mapped to 1..32 distinct regular expressions, each 1..512 bytes.", json!({"type":"object","minProperties":1,"maxProperties":32,"additionalProperties":{"type":"array","minItems":1,"maxItems":32,"uniqueItems":true,"items":{"type":"string","minLength":1,"maxLength":512}}})),
            "paths" => ("array", "Repository-relative glob patterns selecting source files.", strings()),
            "languages" if ["test-naming-strict", "test-annotation-dependency", "file-pattern"].contains(&implementation) => ("array", "Fixed Java syntax scope for this built-in.", json!({"type":"array","const":["java"]})),
            "languages" => ("array", "Syntax adapter languages: java, python, typescript, go, rust, shell.", strings()),
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
        Ok(json!({"name":name,"type":kind,"description":description,"default":defaults[name],"has_default":defaults.get(name).is_some(),"schema":schema}))
    }).collect()
}

fn strings() -> Value {
    json!({"type":"array","items":{"type":"string"}})
}

/// Values are JSON; dotted paths edit one object member while retaining siblings.
pub(super) fn apply(parameters: &mut Value, edits: &[String], implementation: &str) -> Result<()> {
    if edits.len() > 128 {
        bail!("At most 128 parameter edits are allowed");
    }
    if edits.iter().map(String::len).sum::<usize>() > super::MAX_CONFIG_BYTES {
        bail!("Parameter edits exceed 1 MiB");
    }
    let mut paths = Vec::<String>::new();
    let names = names(implementation)?;
    for edit in edits {
        if edit.len() > super::MAX_CONFIG_BYTES {
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
