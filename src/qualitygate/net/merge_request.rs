//! GitHub pull requests and GitLab merge requests share immutable comparison facts.

use super::{Http, validate_url};
use crate::domain::MergeRequest;
use anyhow::{Context, Result, bail};
use reqwest::Url;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct Request {
    pub url: Url,
    pub provider: &'static str,
    pub repository: String,
    pub number: u64,
    api: Url,
}

impl Request {
    pub fn parse(value: &str, api_base: Option<&str>) -> Result<Self> {
        let mut url = Url::parse(value).context("Invalid MR URL")?;
        validate_url(&url)?;
        let segments: Vec<_> = url
            .path_segments()
            .context("MR URL has no repository path")?
            .filter(|part| !part.is_empty())
            .collect();
        let (provider, repo, number) = match segments.as_slice() {
            [owner, repo, "pull", number] => ("github", format!("{owner}/{repo}"), *number),
            [repo @ .., "-", "merge_requests", number] if repo.len() >= 2 => {
                ("gitlab", repo.join("/"), *number)
            }
            _ => bail!(
                "MR URL must identify a GitHub /owner/repo/pull/N or GitLab /group/project/-/merge_requests/N"
            ),
        };
        validate_repository(&repo)?;
        let number: u64 = number
            .parse()
            .context("MR number must be a positive integer")?;
        if number == 0 {
            bail!("MR number must be positive");
        }
        url.set_query(None);
        url.set_fragment(None);
        let mut api = if let Some(base) = api_base {
            let api = Url::parse(base).context("Invalid MR API base URL")?;
            validate_url(&api)?;
            if api.host_str() != url.host_str()
                && !(url.host_str() == Some("github.com")
                    && api.host_str() == Some("api.github.com"))
            {
                bail!("MR API base must use the MR host or GitHub's api.github.com host");
            }
            api
        } else if provider == "github" && url.host_str() == Some("github.com") {
            Url::parse("https://api.github.com/")?
        } else {
            let mut api = url.clone();
            api.set_path(if provider == "github" {
                "/api/v3/"
            } else {
                "/api/v4/"
            });
            api
        };
        if api.query().is_some() || api.fragment().is_some() {
            bail!("MR API base cannot include query or fragment");
        }
        api.set_path(&format!("{}/", api.path().trim_end_matches('/')));
        url.set_query(None);
        url.set_fragment(None);
        Ok(Self {
            url,
            provider,
            repository: repo,
            number,
            api,
        })
    }

    pub fn matches_remote(&self, remote: &str) -> Result<bool> {
        let parsed = if remote.contains("://") {
            Url::parse(remote).context("Invalid origin URL")?
        } else if let Some((host, path)) = remote.split_once(':') {
            let host = host.rsplit('@').next().unwrap_or(host);
            Url::parse(&format!("ssh://{host}/{path}"))?
        } else {
            bail!("MR mode requires a provider URL for origin");
        };
        if !["https", "http", "ssh"].contains(&parsed.scheme())
            || parsed.password().is_some()
            || parsed.scheme() != "ssh" && !parsed.username().is_empty()
        {
            bail!("Origin must use HTTPS or SSH without embedded credentials");
        }
        let repository = parsed.path().trim_matches('/').trim_end_matches(".git");
        let same_repository = if self.provider == "github" {
            repository.eq_ignore_ascii_case(&self.repository)
        } else {
            repository == self.repository
        };
        Ok(parsed.host_str() == self.url.host_str() && same_repository)
    }

    fn endpoint(&self, parts: &[&str]) -> Result<Url> {
        let mut url = self.api.clone();
        url.path_segments_mut()
            .map_err(|_| anyhow::anyhow!("Invalid provider API base"))?
            .pop_if_empty()
            .extend(parts);
        Ok(url)
    }

    pub async fn resolve(&self) -> Result<MergeRequest> {
        let http = Http::new()?;
        let number = self.number.to_string();
        let endpoint = if self.provider == "github" {
            let (owner, repo) = self
                .repository
                .split_once('/')
                .context("Missing repository owner")?;
            self.endpoint(&["repos", owner, repo, "pulls", &number])?
        } else {
            self.endpoint(&["projects", &self.repository, "merge_requests", &number])?
        };
        let response = http.get_provider(endpoint, self.provider).await?;
        let data = &response.value;
        let mut digests = vec![response.digest];
        let (source_branch, target_branch, source_head, target_head) = if self.provider == "github"
        {
            if data["number"].as_u64() != Some(self.number)
                || !field(data, "/base/repo/full_name")?.eq_ignore_ascii_case(&self.repository)
            {
                bail!("GitHub response identifies a different pull request or target repository");
            }
            (
                field(data, "/head/ref")?.to_owned(),
                field(data, "/base/ref")?.to_owned(),
                oid(data, "/head/sha")?,
                oid(data, "/base/sha")?,
            )
        } else {
            if data["iid"].as_u64() != Some(self.number) {
                bail!("GitLab response identifies a different merge request");
            }
            let source = field(data, "/source_branch")?.to_owned();
            let target = field(data, "/target_branch")?.to_owned();
            let head = oid(data, "/sha")?;
            if let Some(diff_head) = data.pointer("/diff_refs/head_sha").and_then(Value::as_str)
                && diff_head != head
            {
                bail!("GitLab diff refs are stale relative to the source head");
            }
            let project = data["target_project_id"]
                .as_u64()
                .context("GitLab target project identity missing")?;
            if data["project_id"].as_u64() != Some(project) {
                bail!("GitLab target project identity mismatch");
            }
            let endpoint = self.endpoint(&[
                "projects",
                &project.to_string(),
                "repository",
                "branches",
                &target,
            ])?;
            let branch = http.get_provider(endpoint, self.provider).await?;
            if field(&branch.value, "/name")? != target {
                bail!("GitLab target branch response mismatch");
            }
            let target_head = oid(&branch.value, "/commit/id")?;
            digests.push(branch.digest);
            (source, target, head, target_head)
        };
        Ok(MergeRequest {
            provider: self.provider.into(),
            url: self.url.to_string(),
            repository: self.repository.clone(),
            number: self.number,
            source_branch,
            target_branch,
            source_head,
            target_head,
            merge_base: String::new(),
            api_response_digests: digests,
        })
    }
}

fn validate_repository(value: &str) -> Result<()> {
    if value.split('/').any(|part| {
        part.is_empty()
            || [".", ".."].contains(&part)
            || !part
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
    }) {
        bail!("Unsupported provider repository path");
    }
    Ok(())
}

fn field<'a>(data: &'a Value, path: &str) -> Result<&'a str> {
    data.pointer(path)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty() && !value.contains(['\0', '\r', '\n']))
        .with_context(|| format!("Provider metadata missing or invalid: {path}"))
}

fn oid(data: &Value, path: &str) -> Result<String> {
    let value = field(data, path)?;
    if ![40, 64].contains(&value.len()) || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        bail!("Provider metadata contains an invalid commit ID: {path}");
    }
    Ok(value.to_ascii_lowercase())
}
