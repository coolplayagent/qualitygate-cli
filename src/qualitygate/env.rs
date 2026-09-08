//! Environment access is centralized; credentials are never included in reports.

use std::path::PathBuf;

pub fn artifact_base() -> PathBuf {
    std::env::var_os("QUALITYGATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("qualitygate"))
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
    use super::token_from;

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
        assert_eq!(token_from("gitlab", "api.github.com", read), None);
        assert_eq!(
            token_from("github", "api.github.com", |_| Some(String::new())),
            None
        );
        assert_eq!(token_from("github", "api.github.com", |_| None), None);
    }
}
