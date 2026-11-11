//! Pure language and bounded regex-map contracts shared by discovery and evaluation.
use anyhow::{Context, Result, ensure};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_PATTERN_LANGUAGES: usize = 32;
pub const MAX_PATTERNS: usize = 32;
pub const MAX_PATTERN_BYTES: usize = 512;
pub type CompiledPatterns = BTreeMap<String, Vec<(String, Regex)>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatternLanguages {
    Syntax,
    Text,
}

impl PatternLanguages {
    pub fn supports(self, language: &str) -> bool {
        crate::rule_catalog::language::named(language).is_some()
            || (self == Self::Text && matches!(language, "c" | "cpp"))
    }

    pub fn names(self) -> Vec<&'static str> {
        let mut languages: Vec<_> = crate::rule_catalog::language::LANGUAGES
            .iter()
            .map(|language| language.name)
            .collect();
        if self == Self::Text {
            languages.extend(["c", "cpp"]);
        }
        languages
    }

    pub fn for_implementation(implementation: &str) -> Self {
        if matches!(
            implementation,
            "file-pattern" | "source-pattern" | "native-syntax"
        ) {
            Self::Text
        } else {
            Self::Syntax
        }
    }
}

pub fn compile_patterns(
    key: &str,
    value: &Value,
    languages: PatternLanguages,
) -> Result<CompiledPatterns> {
    let values = value
        .as_object()
        .with_context(|| format!("{key} must map languages to regex arrays"))?;
    ensure!(
        !values.is_empty() && values.len() <= MAX_PATTERN_LANGUAGES,
        "{key} requires 1..{MAX_PATTERN_LANGUAGES} language entries"
    );
    let mut compiled = BTreeMap::new();
    for (language, entries) in values {
        ensure!(
            language == "all" || languages.supports(language),
            "{key} has unsupported language: {language}"
        );
        let entries = entries
            .as_array()
            .with_context(|| format!("{key}.{language} must be a regex array"))?;
        ensure!(
            !entries.is_empty() && entries.len() <= MAX_PATTERNS,
            "{key}.{language} requires 1..{MAX_PATTERNS} regex patterns"
        );
        let mut unique = BTreeSet::new();
        let mut patterns = Vec::new();
        for value in entries {
            let pattern = value
                .as_str()
                .with_context(|| format!("{key}.{language} values must be strings"))?;
            ensure!(
                !pattern.is_empty() && pattern.len() <= MAX_PATTERN_BYTES && unique.insert(pattern),
                "{key}.{language} requires distinct regex patterns of 1..{MAX_PATTERN_BYTES} bytes"
            );
            patterns.push((pattern.to_owned(), Regex::new(pattern)?));
        }
        compiled.insert(language.to_owned(), patterns);
    }
    Ok(compiled)
}

pub fn pattern_schema(languages: PatternLanguages) -> Value {
    let mut names = languages.names();
    names.push("all");
    json!({"type":"object","minProperties":1,"maxProperties":MAX_PATTERN_LANGUAGES,
        "propertyNames":{"enum":names},"additionalProperties":{"type":"array","minItems":1,"maxItems":MAX_PATTERNS,
        "uniqueItems":true,"items":{"type":"string","minLength":1,"maxLength":MAX_PATTERN_BYTES}}})
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LanguageScope {
    Fixed,
    Subset,
}

pub struct LanguageContract<'a> {
    scope: Option<LanguageScope>,
    declared: Vec<&'a str>,
    patterns: bool,
    supported: PatternLanguages,
}

impl<'a> LanguageContract<'a> {
    pub fn builtin(
        implementation: &str,
        declared: &'a [String],
        explicit: Option<LanguageScope>,
    ) -> Result<Self> {
        let intrinsic = match implementation {
            "test-naming-strict" | "test-annotation-dependency" => Some(vec!["java"]),
            "shell-shebang" | "shell-commented-code" => Some(vec!["shell"]),
            _ => None,
        };
        ensure!(
            explicit != Some(LanguageScope::Subset)
                || (implementation == "source-pattern" && !declared.is_empty()),
            "Subset language scope requires source-pattern and declared languages"
        );
        ensure!(
            explicit != Some(LanguageScope::Fixed) || !declared.is_empty(),
            "Fixed language scope requires declared languages"
        );
        let fixed = intrinsic.is_some()
            || matches!(
                implementation,
                "file-pattern" | "native-syntax" | "language-syntax"
            )
            || (implementation == "source-pattern" && !declared.is_empty());
        let scope = explicit.or_else(|| fixed.then_some(LanguageScope::Fixed));
        let declared = intrinsic.unwrap_or_else(|| declared.iter().map(String::as_str).collect());
        Ok(Self {
            scope,
            declared,
            patterns: implementation == "source-pattern",
            supported: PatternLanguages::for_implementation(implementation),
        })
    }

    pub fn validate(&self, id: &str, parameters: &BTreeMap<String, Value>) -> Result<()> {
        match self.scope {
            Some(LanguageScope::Fixed) => {
                ensure!(
                    parameters.get("languages") == Some(&json!(self.declared)),
                    "{id} requires languages: {:?}",
                    self.declared
                );
                if self.patterns {
                    let patterns = parameters
                        .get("prohibited_patterns")
                        .and_then(Value::as_object);
                    ensure!(
                        patterns.is_some_and(|patterns| patterns.len() == self.declared.len()
                            && self
                                .declared
                                .iter()
                                .all(|language| patterns.contains_key(*language))),
                        "{id} requires prohibited_patterns for exactly {:?}",
                        self.declared
                    );
                }
            }
            Some(LanguageScope::Subset) => {
                let languages = parameters
                    .get("languages")
                    .and_then(Value::as_array)
                    .with_context(|| {
                        format!(
                            "{id} requires a nonempty languages array within its declared scope"
                        )
                    })?;
                ensure!(
                    !languages.is_empty()
                        && languages.iter().all(|language| language
                            .as_str()
                            .is_some_and(|language| self.declared.contains(&language))),
                    "{id} requires a nonempty languages array within its declared scope"
                );
                if let Some(patterns) = parameters.get("prohibited_patterns") {
                    let patterns = patterns
                        .as_object()
                        .context("prohibited_patterns must map languages to regex arrays")?;
                    ensure!(
                        patterns.keys().all(|language| language == "all"
                            || self.declared.contains(&language.as_str())),
                        "{id} requires patterns within its declared language scope"
                    );
                }
            }
            None => {}
        }
        Ok(())
    }

    pub fn language_schema(&self) -> Value {
        match self.scope {
            Some(LanguageScope::Fixed) => json!({"type":"array","const":self.declared}),
            Some(LanguageScope::Subset) => {
                json!({"type":"array","minItems":1,"items":{"type":"string","enum":self.declared}})
            }
            None => json!({"type":"array","items":{"type":"string","enum":self.supported.names()}}),
        }
    }

    pub fn requires_languages(&self) -> bool {
        self.scope.is_some()
    }

    pub fn pattern_schema(&self) -> Value {
        let mut schema = pattern_schema(self.supported);
        if let Some(scope) = self.scope {
            let mut keys = self.declared.clone();
            if scope == LanguageScope::Fixed {
                schema["required"] = json!(keys);
            } else {
                keys.push("all");
            }
            schema["propertyNames"] = json!({"enum":keys});
        }
        schema
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pattern_contract_preserves_limits_and_separates_text_from_syntax_languages() {
        for bad in [
            json!({}),
            json!([]),
            json!({"ruby":["x"]}),
            json!({"rust":"x"}),
            json!({"rust":[]}),
            json!({"rust":[1]}),
            json!({"rust":[""]}),
            json!({"rust":["x","x"]}),
            json!({"rust":["["]}),
            json!({"rust":["界".repeat(171)]}),
            json!({"rust":vec!["x";33]}),
        ] {
            assert!(
                compile_patterns("patterns", &bad, PatternLanguages::Text).is_err(),
                "{bad}"
            );
        }
        let many = Value::Object(
            (0..33)
                .map(|i| (format!("lang{i}"), json!(["x"])))
                .collect(),
        );
        assert!(compile_patterns("patterns", &many, PatternLanguages::Text).is_err());
        let c = json!({"c":["x"],"cpp":["y"],"all":["z"]});
        assert!(compile_patterns("patterns", &c, PatternLanguages::Syntax).is_err());
        let compiled = compile_patterns("patterns", &c, PatternLanguages::Text).unwrap();
        assert!(compiled["c"][0].1.is_match("x"));
        compile_patterns(
            "patterns",
            &json!({"rust":["x".repeat(512)]}),
            PatternLanguages::Syntax,
        )
        .unwrap();
    }

    #[test]
    fn scopes_come_from_definitions_and_keep_fixed_and_subset_contracts_distinct() {
        let declared = vec!["c".into(), "cpp".into()];
        let fixed = LanguageContract::builtin("source-pattern", &declared, None).unwrap();
        let good = serde_json::from_value(
            json!({"languages":["c","cpp"],"prohibited_patterns":{"c":["x"],"cpp":["x"]}}),
        )
        .unwrap();
        fixed.validate("renamed-rule", &good).unwrap();
        assert_eq!(fixed.language_schema()["const"], json!(["c", "cpp"]));
        assert_eq!(fixed.pattern_schema()["required"], json!(["c", "cpp"]));
        for bad in [
            json!({"languages":["cpp","c"],"prohibited_patterns":{"c":["x"],"cpp":["x"]}}),
            json!({"languages":["c","cpp"],"prohibited_patterns":{"all":["x"]}}),
        ] {
            assert!(
                fixed
                    .validate("renamed-rule", &serde_json::from_value(bad).unwrap())
                    .is_err()
            );
        }
        let subset =
            LanguageContract::builtin("source-pattern", &declared, Some(LanguageScope::Subset))
                .unwrap();
        subset
            .validate(
                "review",
                &serde_json::from_value(
                    json!({"languages":["c"],"prohibited_patterns":{"all":["x"]}}),
                )
                .unwrap(),
            )
            .unwrap();
        assert!(
            subset
                .validate(
                    "review",
                    &serde_json::from_value(json!({"languages":["rust"]})).unwrap()
                )
                .is_err()
        );
        assert!(
            subset
                .validate(
                    "review",
                    &serde_json::from_value(
                        json!({"languages":["c"],"prohibited_patterns":{"rust":["x"]}})
                    )
                    .unwrap()
                )
                .is_err()
        );
        for invalid in [
            json!({"prohibited_patterns":{"all":["x"]}}),
            json!({"languages":[],"prohibited_patterns":{"all":["x"]}}),
            json!({"languages":"c","prohibited_patterns":{"all":["x"]}}),
            json!({"languages":["c"],"prohibited_patterns":[]}),
        ] {
            assert!(
                subset
                    .validate("review", &serde_json::from_value(invalid).unwrap())
                    .is_err()
            );
        }
        assert!(subset.requires_languages());
        assert_eq!(subset.language_schema()["minItems"], 1);
        assert_eq!(
            subset.pattern_schema()["propertyNames"]["enum"],
            json!(["c", "cpp", "all"])
        );
        assert!(
            LanguageContract::builtin("test-naming", &declared, Some(LanguageScope::Subset))
                .is_err()
        );
        assert!(
            LanguageContract::builtin("source-pattern", &[], Some(LanguageScope::Fixed)).is_err()
        );
        assert!(
            LanguageContract::builtin("source-pattern", &[], Some(LanguageScope::Subset)).is_err()
        );
        let generic = LanguageContract::builtin("source-pattern", &[], None).unwrap();
        assert!(!generic.requires_languages());
        generic.validate("review", &BTreeMap::new()).unwrap();
        assert!(
            generic.language_schema()["items"]["enum"]
                .as_array()
                .unwrap()
                .contains(&json!("cpp"))
        );
        assert_eq!(
            generic.pattern_schema(),
            pattern_schema(PatternLanguages::Text)
        );
        assert!(
            subset.language_schema()["items"]["enum"]
                .as_array()
                .unwrap()
                .contains(&json!("c"))
        );
        assert_eq!(
            LanguageContract::builtin("test-naming-strict", &[], None)
                .unwrap()
                .language_schema()["const"],
            json!(["java"])
        );
        assert_eq!(
            LanguageContract::builtin("shell-shebang", &[], None)
                .unwrap()
                .language_schema()["const"],
            json!(["shell"])
        );
    }
}
