//! Bounded provider HTTP access, with host-bound credentials and no redirects.

pub mod merge_request;

use anyhow::{Context, Result, bail};
use reqwest::{
    Client, Url,
    header::{AUTHORIZATION, HeaderValue},
};
use sha2::{Digest, Sha256};
use std::time::Duration;

pub const MAX_RESPONSE_BYTES: usize = 1_048_576;

pub struct JsonResponse {
    pub value: serde_json::Value,
    pub digest: String,
}

pub struct Http {
    client: Client,
}

impl Http {
    pub async fn get_provider(&self, url: Url, provider: &str) -> Result<JsonResponse> {
        let host = url.host_str().unwrap_or_default();
        let authority = url
            .port()
            .map_or_else(|| host.to_owned(), |port| format!("{host}:{port}"));
        let credential = crate::env::provider_token(provider, &authority);
        self.get(url, credential).await
    }

    pub fn new() -> Result<Self> {
        Self::with_deadline(Duration::from_secs(30))
    }

    fn with_deadline(deadline: Duration) -> Result<Self> {
        Ok(Self {
            client: Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(Duration::from_secs(5))
                .timeout(deadline)
                .user_agent(concat!("qualitygate/", env!("CARGO_PKG_VERSION")))
                .build()?,
        })
    }

    pub async fn get(&self, url: Url, credential: Option<String>) -> Result<JsonResponse> {
        validate_url(&url)?;
        let mut request = self
            .client
            .get(url.clone())
            .header("Accept", "application/json");
        if let Some(credential) = credential {
            if url.scheme() != "https" {
                bail!("Credentials require HTTPS");
            }
            let mut header = HeaderValue::from_str(&format!("Bearer {credential}"))
                .context("Invalid provider credential")?;
            header.set_sensitive(true);
            request = request.header(AUTHORIZATION, header);
        }
        let mut response = request.send().await.context("Provider request failed")?;
        if !response.status().is_success() {
            bail!(
                "Provider returned HTTP {} for {}; redirects are not followed",
                response.status().as_u16(),
                url.path()
            );
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
        {
            bail!("Provider response exceeds 1 MiB");
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            if bytes.len() + chunk.len() > MAX_RESPONSE_BYTES {
                bail!("Provider response exceeds 1 MiB");
            }
            bytes.extend_from_slice(&chunk);
        }
        let value =
            serde_json::from_slice(&bytes).context("Provider response is not valid JSON")?;
        Ok(JsonResponse {
            value,
            digest: format!("sha256:{:x}", Sha256::digest(&bytes)),
        })
    }
}

pub fn validate_url(url: &Url) -> Result<()> {
    if !url.username().is_empty() || url.password().is_some() || url.host_str().is_none() {
        bail!("Provider URL must have a host and no embedded credentials");
    }
    let loopback = url.host_str().is_some_and(|host| {
        host == "localhost"
            || host
                .trim_matches(['[', ']'])
                .parse::<std::net::IpAddr>()
                .is_ok_and(|address| address.is_loopback())
    });
    if url.scheme() != "https" && !(url.scheme() == "http" && loopback) {
        bail!("Provider URL requires HTTPS (HTTP is supported only on loopback)");
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../../tests/common/http.rs"]
mod test_server;
#[cfg(test)]
mod tests;
