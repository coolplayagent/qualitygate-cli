//! Core Metadata headers are parsed separately from pip's JSON transport.

use anyhow::{Context, Result, bail};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub(super) struct Metadata {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub requires_dist: Vec<String>,
    #[serde(default)]
    pub requires_python: Option<String>,
    #[serde(default)]
    pub provides_extra: Vec<String>,
}

pub(super) fn parse(bytes: &[u8]) -> Result<Metadata> {
    let text = std::str::from_utf8(bytes)?;
    let mut headers: Vec<(String, String)> = Vec::new();
    for line in text.lines() {
        if line.is_empty() {
            break;
        }
        if line.starts_with([' ', '\t']) {
            let previous = headers
                .last_mut()
                .context("Metadata continuation without a header")?;
            previous.1.push(' ');
            previous.1.push_str(line.trim());
        } else {
            let (name, value) = line
                .split_once(':')
                .context("Malformed installed METADATA header")?;
            headers.push((name.to_ascii_lowercase(), value.trim().into()));
        }
    }
    let all = |name: &str| {
        headers
            .iter()
            .filter(|(key, _)| key == name)
            .map(|(_, value)| value.clone())
            .collect::<Vec<_>>()
    };
    let single = |name: &str| -> Result<Option<String>> {
        let values = all(name);
        if values.len() > 1 {
            bail!("Duplicate installed METADATA {name}");
        }
        Ok(values.into_iter().next())
    };
    let version = single("metadata-version")?.context("Missing Metadata-Version")?;
    if !matches!(
        version.as_str(),
        "1.2" | "2.1" | "2.2" | "2.3" | "2.4" | "2.5"
    ) {
        bail!("Unsupported Core Metadata version: {version}");
    }
    Ok(Metadata {
        name: single("name")?.context("Missing installed distribution name")?,
        version: single("version")?.context("Missing installed distribution version")?,
        requires_dist: all("requires-dist"),
        requires_python: single("requires-python")?,
        provides_extra: all("provides-extra"),
    })
}
