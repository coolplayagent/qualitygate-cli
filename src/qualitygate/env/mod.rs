//! Environment access is centralized; credentials are never included in reports.

use std::path::PathBuf;

pub const BUILTIN_RULES_DIR_ENV: &str = "QUALITYGATE_BUILTIN_RULES_DIR";

pub fn artifact_base() -> PathBuf {
    std::env::var_os("QUALITYGATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("qualitygate"))
}

pub fn executable(program: &str, cwd: &std::path::Path) -> anyhow::Result<PathBuf> {
    use anyhow::Context;
    which::which_in(program, std::env::var_os("PATH"), cwd)
        .with_context(|| format!("Cannot resolve executable {program}"))
}

pub fn platform() -> (&'static str, &'static str) {
    (std::env::consts::OS, std::env::consts::ARCH)
}

/// Values never leave this boundary, including non-UTF-8 inherited values.
pub fn required_present(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|value| !value.is_empty())
}

pub fn missing_required(names: &[String]) -> Vec<String> {
    names
        .iter()
        .filter(|name| !required_present(name))
        .cloned()
        .collect()
}

pub fn current_executable() -> anyhow::Result<PathBuf> {
    Ok(std::env::current_exe()?)
}

/// Bind inherited process inputs without disclosing environment values.
pub fn environment_digest() -> anyhow::Result<String> {
    digest_environment(std::env::vars_os().collect())
}

fn digest_environment(
    mut variables: Vec<(std::ffi::OsString, std::ffi::OsString)>,
) -> anyhow::Result<String> {
    use sha2::{Digest, Sha256};
    variables.sort();
    let total: usize = variables
        .iter()
        .map(|(key, value)| key.as_encoded_bytes().len() + value.as_encoded_bytes().len())
        .sum();
    if variables.len() > 4096 || total > 1024 * 1024 {
        anyhow::bail!("Environment identity exceeds 4096 variables or 1 MiB");
    }
    let mut hash = Sha256::new();
    hash.update(std::env::consts::OS);
    hash.update(std::env::consts::ARCH);
    for (key, value) in variables {
        for bytes in [key.as_encoded_bytes(), value.as_encoded_bytes()] {
            hash.update((bytes.len() as u64).to_le_bytes());
            hash.update(bytes);
        }
    }
    Ok(format!("sha256:{:x}", hash.finalize()))
}

/// Native selfcheck must not let inherited Git overrides redirect temporary
/// repository writes into another repository or object/index store.
pub fn require_isolated_git_environment() -> anyhow::Result<()> {
    for name in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_COMMON_DIR",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_NAMESPACE",
        "GIT_CONFIG",
        "GIT_CONFIG_COUNT",
        "GIT_CONFIG_PARAMETERS",
    ] {
        if std::env::var_os(name).is_some() {
            anyhow::bail!(
                "Native fixture cannot safely isolate Git while {name} is set; run selfcheck without that override"
            );
        }
    }
    Ok(())
}

/// Returns the caller-selected Skill asset directory, if any.
pub fn builtin_rule_assets_override() -> Option<PathBuf> {
    std::env::var_os(BUILTIN_RULES_DIR_ENV).map(PathBuf::from)
}

/// Finds deployment-owned Skill rule asset locations without inspecting a
/// repository working directory, whose content belongs to policy instead.
pub fn builtin_rule_asset_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(executable) = std::env::current_exe()
        && let Some(skill_root) = executable.ancestors().nth(3)
    {
        candidates.push(skill_root.join("references/rules"));
    }
    for variable in ["TEST_SRCDIR", "RUNFILES_DIR"] {
        if let Some(root) = std::env::var_os(variable) {
            let root = PathBuf::from(root);
            if let Some(workspace) = std::env::var_os("TEST_WORKSPACE") {
                candidates.push(
                    root.join(workspace)
                        .join("skills/qualitygate-cli/references/rules"),
                );
            }
            candidates.push(root.join("_main/skills/qualitygate-cli/references/rules"));
        }
    }
    candidates.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("skills/qualitygate-cli/references/rules"),
    );
    candidates
}

pub fn provider_token(provider: &str, host: &str) -> Option<String> {
    token_from(provider, host, |key| std::env::var(key).ok())
}

fn token_from(provider: &str, host: &str, read: impl Fn(&str) -> Option<String>) -> Option<String> {
    if read("QUALITYGATE_MR_TOKEN_HOST").is_some_and(|bound| bound == host) {
        return read("QUALITYGATE_MR_TOKEN").filter(|value| !value.is_empty());
    }
    let key = match (provider, host) {
        ("github", "api.github.com") => "GITHUB_TOKEN",
        ("gitlab", "gitlab.com") => "GITLAB_TOKEN",
        _ => return None,
    };
    read(key).filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::{digest_environment, token_from};

    #[test]
    fn environment_identity_enforces_exact_count_and_byte_limits() {
        // An OS may reject oversized process environments before Rust runs.
        let at_count_limit = (0..4096)
            .map(|index| (format!("KEY_{index}").into(), "x".into()))
            .collect::<Vec<_>>();
        assert!(digest_environment(at_count_limit.clone()).is_ok());
        let mut above_count_limit = at_count_limit;
        above_count_limit.push(("KEY_4096".into(), "x".into()));
        assert_eq!(
            digest_environment(above_count_limit)
                .unwrap_err()
                .to_string(),
            "Environment identity exceeds 4096 variables or 1 MiB"
        );

        let key = "BOUND";
        let value = "x".repeat(1024 * 1024 - key.len());
        assert!(digest_environment(vec![(key.into(), value.clone().into())]).is_ok());
        assert_eq!(
            digest_environment(vec![(key.into(), format!("{value}x").into())])
                .unwrap_err()
                .to_string(),
            "Environment identity exceeds 4096 variables or 1 MiB"
        );
    }

    #[test]
    fn environment_identity_sorts_and_frames_inherited_values() {
        let variables = vec![("B".into(), "two".into()), ("A".into(), "one".into())];
        let mut reversed = variables.clone();
        reversed.reverse();
        assert_eq!(
            digest_environment(variables).unwrap(),
            digest_environment(reversed).unwrap()
        );
        assert_ne!(
            digest_environment(vec![("ab".into(), "c".into())]).unwrap(),
            digest_environment(vec![("a".into(), "bc".into())]).unwrap()
        );
    }

    #[test]
    fn provider_credentials_are_bound_to_exact_api_authorities() {
        let values = std::collections::BTreeMap::from([
            ("GITHUB_TOKEN", "github"),
            ("GITLAB_TOKEN", "gitlab"),
            ("QUALITYGATE_MR_TOKEN", "private"),
            ("QUALITYGATE_MR_TOKEN_HOST", "git.example:8443"),
        ]);
        let read = |key: &str| values.get(key).map(|value| (*value).to_owned());
        assert_eq!(
            token_from("github", "api.github.com", read).as_deref(),
            Some("github")
        );
        assert_eq!(
            token_from("gitlab", "gitlab.com", read).as_deref(),
            Some("gitlab")
        );
        assert_eq!(
            token_from("gitlab", "git.example:8443", read).as_deref(),
            Some("private")
        );
        for host in [
            "github.com",
            "api.github.com:8443",
            "git.example",
            "other.example",
        ] {
            assert_eq!(token_from("github", host, read), None);
        }
        assert_eq!(
            token_from("gitlab", "api.github.com", |_| Some(String::new())),
            None
        );
        assert_eq!(token_from("github", "api.github.com", |_| None), None);
    }
}

#[cfg(test)]
mod process_scenarios {
    use super::*;

    #[test]
    fn inherited_inputs_are_bounded_and_do_not_disclose_credentials() {
        if let Ok(case) = std::env::var("QUALITYGATE_ENV_UNIT_CASE") {
            assert_eq!(case, "overrides");
            assert!(
                require_isolated_git_environment()
                    .unwrap_err()
                    .to_string()
                    .contains("GIT_INDEX_FILE")
            );
            assert_eq!(artifact_base(), PathBuf::from("/unit/artifacts"));
            assert_eq!(
                builtin_rule_assets_override(),
                Some(PathBuf::from("/unit/rules"))
            );
            assert!(builtin_rule_asset_candidates().contains(&PathBuf::from(
                "/unit/runfiles/workspace/skills/qualitygate-cli/references/rules"
            )));
            assert!(required_present("QUALITYGATE_ENV_UNIT_CASE"));
            assert!(!required_present("QUALITYGATE_UNIT_EMPTY"));
            assert_eq!(
                missing_required(&["QUALITYGATE_UNIT_EMPTY".into()]),
                vec!["QUALITYGATE_UNIT_EMPTY"]
            );
            let identity = environment_digest().unwrap();
            assert_eq!(identity.len(), 71);
            assert!(!identity.contains("overrides"));
            return;
        }
        let output = std::process::Command::new(current_executable().unwrap())
            .args(["--exact", "env::process_scenarios::inherited_inputs_are_bounded_and_do_not_disclose_credentials", "--nocapture"])
            .env("QUALITYGATE_ENV_UNIT_CASE", "overrides")
            .env("GIT_INDEX_FILE", "/unit/index")
            .env("QUALITYGATE_HOME", "/unit/artifacts")
            .env(BUILTIN_RULES_DIR_ENV, "/unit/rules")
            .env("RUNFILES_DIR", "/unit/runfiles")
            .env("TEST_WORKSPACE", "workspace")
            .env("QUALITYGATE_UNIT_EMPTY", "")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "overrides: {}",
            String::from_utf8_lossy(&output.stdout)
        );
    }
}
