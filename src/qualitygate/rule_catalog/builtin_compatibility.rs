//! Read-only interpretation of pre-metadata builtins retained in frozen policies.
//! These historical exceptions do not rewrite serialized definitions or bindings.
use crate::domain::{rule_guidance::Diagnostic, rule_parameters::LanguageScope};
use crate::rule_catalog::catalog::Builtin;
use std::borrow::Cow;

impl Builtin {
    pub fn effective_language_scope(&self) -> Option<LanguageScope> {
        self.language_scope
            .or(match (self.id.as_str(), self.implementation.as_str()) {
                ("security-sensitive-api", "source-pattern") => Some(LanguageScope::Subset),
                ("python-test-naming", "test-naming") => Some(LanguageScope::Fixed),
                _ => None,
            })
    }

    pub fn effective_diagnostic(&self) -> Option<Cow<'_, Diagnostic>> {
        if let Some(diagnostic) = &self.diagnostic {
            return Some(Cow::Borrowed(diagnostic));
        }
        if self.implementation != "source-pattern" {
            return None;
        }
        let (message, fix) = match self.id.as_str() {
            "cpp-disabled-test" => (
                "Disabled-test marker requires review; this is a review signal, not proof of missing coverage",
                "Confirm the reason, tracking issue, and replacement coverage; restore the test when appropriate. Tracking text alone does not suppress this marker signal",
            ),
            "c-ignored-atomic-cas" => (
                "Configured compare-exchange result discard requires review; this is a review signal, not proof of a data race or lost update",
                "Verify the selected API's success and failure semantics and handle failure or document why ignoring it is valid; check retry behavior and supporting tests",
            ),
            "workaround-marker" => (
                "Temporary-workaround marker requires review; this is a review signal, not proof of a defect",
                "Confirm a root-cause issue, an owner or expiry condition, and a removal plan; remove the workaround only after fixing its cause. Tracking text alone does not suppress this marker signal",
            ),
            _ => return None,
        };
        Some(Cow::Owned(Diagnostic {
            message: message.into(),
            fix: fix.into(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::rule_parameters::LanguageContract;
    use serde_json::json;

    fn archived(id: &str, implementation: &str, languages: &[&str]) -> Builtin {
        serde_json::from_value(json!({"id":id,"version":1,"description":"historical rule","implementation":implementation,"language":languages,"requires_capabilities":["files"],"defaults":{"required":true,"severity":"warning"}})).unwrap()
    }

    #[test]
    fn frozen_language_contracts_resolve_without_mutating_their_serialized_definition() {
        for (id, implementation, languages, valid, invalid) in [
            (
                "security-sensitive-api",
                "source-pattern",
                vec!["java", "python"],
                json!({"languages":["python"],"prohibited_patterns":{"all":["x"]}}),
                json!({"languages":["rust"],"prohibited_patterns":{"all":["x"]}}),
            ),
            (
                "python-test-naming",
                "test-naming",
                vec!["python"],
                json!({"languages":["python"]}),
                json!({"languages":["java"]}),
            ),
        ] {
            let rule = archived(id, implementation, &languages);
            assert!(rule.language_scope.is_none());
            let before = serde_json::to_vec(&rule).unwrap();
            let contract = LanguageContract::builtin(
                implementation,
                &rule.language,
                rule.effective_language_scope(),
            )
            .unwrap();
            contract
                .validate(id, &serde_json::from_value(valid.clone()).unwrap())
                .unwrap();
            assert!(
                contract
                    .validate(id, &serde_json::from_value(invalid.clone()).unwrap())
                    .is_err()
            );
            let catalog = crate::config::catalog::Catalog {
                entries: [(
                    id.into(),
                    crate::config::catalog::Entry {
                        origin: "frozen".into(),
                        package: "shared".into(),
                        builtin: Some(rule.clone()),
                        custom: None,
                    },
                )]
                .into(),
            };
            let mut config = crate::config::Config::default();
            config.rules.insert(
                id.into(),
                serde_json::from_value(json!({"parameters":valid})).unwrap(),
            );
            catalog.resolve(&config).unwrap();
            config.rules.insert(
                id.into(),
                serde_json::from_value(json!({"parameters":invalid})).unwrap(),
            );
            assert!(catalog.resolve(&config).is_err());
            assert_eq!(before, serde_json::to_vec(&rule).unwrap());
            assert!(
                serde_json::to_value(&rule)
                    .unwrap()
                    .get("language_scope")
                    .is_none()
            );
        }
        let mut explicit = archived("security-sensitive-api", "source-pattern", &["java"]);
        explicit.language_scope = Some(LanguageScope::Fixed);
        assert_eq!(
            explicit.effective_language_scope(),
            Some(LanguageScope::Fixed)
        );
        assert_eq!(
            archived("other", "source-pattern", &["java"]).effective_language_scope(),
            None
        );
    }

    #[test]
    fn frozen_review_guidance_is_preserved_but_explicit_metadata_remains_authoritative() {
        for id in [
            "cpp-disabled-test",
            "c-ignored-atomic-cas",
            "workaround-marker",
        ] {
            let mut rule = archived(id, "source-pattern", &[]);
            let before = serde_json::to_vec(&rule).unwrap();
            let diagnostic = rule.effective_diagnostic().unwrap();
            assert!(diagnostic.message.contains("review signal, not proof"));
            diagnostic.validate().unwrap();
            assert_eq!(before, serde_json::to_vec(&rule).unwrap());
            assert!(
                serde_json::to_value(&rule)
                    .unwrap()
                    .get("diagnostic")
                    .is_none()
            );
            rule.diagnostic = Some(Diagnostic {
                message: "Reviewed wording".into(),
                fix: "Reviewed remediation".into(),
            });
            assert!(matches!(
                rule.effective_diagnostic(),
                Some(Cow::Borrowed(_))
            ));
            assert_eq!(
                rule.effective_diagnostic().unwrap().message,
                "Reviewed wording"
            );
            rule.diagnostic = None;
            rule.implementation = "line-ending".into();
            assert!(rule.effective_diagnostic().is_none());
        }
        assert!(
            archived("unrelated", "source-pattern", &[])
                .effective_diagnostic()
                .is_none()
        );
    }

    #[test]
    fn subset_scope_inherits_declared_languages_but_rejects_an_explicit_empty_override() {
        let archived = archived(
            "security-sensitive-api",
            "source-pattern",
            &["java", "python"],
        );
        for scope in [None, Some(LanguageScope::Subset)] {
            let mut rule = archived.clone();
            rule.language_scope = scope;
            let descriptors = crate::config::parameters::describe_builtin(&rule).unwrap();
            let languages = descriptors
                .iter()
                .find(|item| item["name"] == "languages")
                .unwrap();
            assert_eq!(languages["required"], true);
            assert_eq!(languages["schema"]["minItems"], 1);
            let catalog = crate::config::catalog::Catalog {
                entries: [(
                    rule.id.clone(),
                    crate::config::catalog::Entry {
                        origin: "frozen".into(),
                        package: "shared".into(),
                        builtin: Some(rule),
                        custom: None,
                    },
                )]
                .into(),
            };
            let mut config = crate::config::Config::default();
            config.rules.insert(
                "security-sensitive-api".into(),
                serde_json::from_value(
                    json!({"parameters":{"prohibited_patterns":{"all":["risk"]}}}),
                )
                .unwrap(),
            );
            let resolved = catalog.resolve(&config).unwrap();
            assert_eq!(
                resolved.rules["security-sensitive-api"].parameters["languages"],
                json!(["java", "python"])
            );
            config
                .rules
                .get_mut("security-sensitive-api")
                .unwrap()
                .parameters
                .insert("languages".into(), json!([]));
            assert!(
                catalog
                    .resolve(&config)
                    .unwrap_err()
                    .to_string()
                    .contains("nonempty languages")
            );
        }
    }
}
