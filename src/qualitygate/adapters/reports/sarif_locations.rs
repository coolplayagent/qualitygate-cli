//! Resolve SARIF location tables and file URIs without filesystem or network I/O.

use super::{
    IssueLocation,
    json::{array, object, optional_text, text},
};
use anyhow::{Context, Result, bail};
use serde_json::Value;
use url::Url;

const RELATIVE_ROOT: &str = "file:///__qualitygate_relative__/";
const MAX_DEPTH: usize = 32;

#[derive(PartialEq)]
struct ResolvedUri {
    url: Url,
    relative: bool,
}

fn index(value: &Value, key: &str) -> Result<Option<usize>> {
    match value.get(key) {
        None => Ok(None),
        Some(value) if value.as_i64() == Some(-1) => Ok(None),
        Some(value) => Ok(Some(usize::try_from(
            value.as_u64().context("Invalid SARIF table index")?,
        )?)),
    }
}

fn entry<'a>(run: &'a Value, table: &str, index: usize) -> Result<&'a Value> {
    array(run, table)?
        .get(index)
        .with_context(|| format!("SARIF {table} index is out of bounds"))
}

fn step(depth: usize, budget: &mut usize) -> Result<()> {
    if depth > MAX_DEPTH {
        bail!("SARIF location reference depth exceeds 32; cyclic or excessive references");
    }
    *budget = budget
        .checked_sub(1)
        .context("SARIF location resolution work budget exhausted")?;
    Ok(())
}

fn resolve(run: &Value, location: &Value, depth: usize, budget: &mut usize) -> Result<ResolvedUri> {
    step(depth, budget)?;
    object(location)?;
    let referenced = index(location, "index")?
        .map(|index| {
            let value = entry(run, "artifacts", index)?
                .get("location")
                .context("SARIF artifact has no location")?;
            resolve(run, value, depth + 1, budget)
        })
        .transpose()?;
    let own = optional_text(location, "uri")?
        .map(|uri| {
            if uri.len() > 16 * 1024 {
                bail!("SARIF URI exceeds 16 KiB");
            }
            let bytes = uri.as_bytes();
            for (offset, byte) in bytes.iter().enumerate() {
                if *byte == b'%'
                    && (offset + 2 >= bytes.len()
                        || !bytes[offset + 1].is_ascii_hexdigit()
                        || !bytes[offset + 2].is_ascii_hexdigit())
                {
                    bail!("Invalid percent escape in SARIF URI");
                }
            }
            if uri.trim() != uri
                || uri.contains('\\')
                || uri.bytes().any(|byte| byte.is_ascii_control())
            {
                bail!("SARIF locations require portable file URIs");
            }
            let base = if let Some(id) = optional_text(location, "uriBaseId")? {
                let bases = run
                    .get("originalUriBaseIds")
                    .context("SARIF URI base table is missing")?;
                let base = object(bases)?
                    .get(id)
                    .context("SARIF URI base ID is unresolved")?;
                resolve(run, base, depth + 1, budget)?
            } else {
                ResolvedUri {
                    url: Url::parse(RELATIVE_ROOT)?,
                    relative: true,
                }
            };
            let url = base.url.join(uri)?;
            if url.as_str().len() > 16 * 1024 {
                bail!("Resolved SARIF URI exceeds 16 KiB");
            }
            if url.scheme() != "file"
                || url.host_str().is_some_and(|host| host != "localhost")
                || url.query().is_some()
                || url.fragment().is_some()
            {
                bail!(
                    "SARIF diagnostics must refer to local file URIs without queries or fragments"
                );
            }
            let relative = base.relative && Url::parse(uri).is_err();
            if relative && !url.as_str().starts_with(RELATIVE_ROOT) {
                bail!("Relative SARIF URI escapes the source root");
            }
            Ok::<_, anyhow::Error>(ResolvedUri { url, relative })
        })
        .transpose()?;
    match (own, referenced) {
        (Some(own), Some(referenced)) if own != referenced => {
            bail!("SARIF artifact index contradicts its URI")
        }
        (Some(own), _) => Ok(own),
        (_, Some(referenced)) => Ok(referenced),
        _ => bail!("SARIF artifact location needs a URI or a resolvable index"),
    }
}

fn file(run: &Value, location: &Value, budget: &mut usize) -> Result<String> {
    let resolved = resolve(run, location, 0, budget)?;
    let decoded = percent_encoding::percent_decode_str(resolved.url.path()).decode_utf8()?;
    if decoded.contains(['\\', '\0', '\r', '\n']) {
        bail!("SARIF URI decodes to an unsafe source path");
    }
    let path = if resolved.relative {
        decoded
            .strip_prefix("/__qualitygate_relative__/")
            .context("Invalid relative source URI")?
            .to_owned()
    } else {
        resolved
            .url
            .to_file_path()
            .map_err(|()| anyhow::anyhow!("Invalid SARIF file URI"))?
            .to_str()
            .context("SARIF paths must be UTF-8")?
            .to_owned()
    };
    if path.is_empty() || path.contains(['\0', '\r', '\n']) {
        bail!("SARIF location has no valid source path");
    }
    Ok(path)
}

fn logical(run: &Value, value: &Value, depth: usize, budget: &mut usize) -> Result<Option<String>> {
    step(depth, budget)?;
    object(value)?;
    let own = optional_text(value, "fullyQualifiedName")?.map(str::to_owned);
    let referenced = index(value, "index")?
        .map(|index| {
            logical(
                run,
                entry(run, "logicalLocations", index)?,
                depth + 1,
                budget,
            )
        })
        .transpose()?
        .flatten();
    if own.is_some() && referenced.is_some() && own != referenced {
        bail!("SARIF logical index contradicts its symbol");
    }
    Ok(own.or(referenced))
}

pub(super) fn locations(
    run: &Value,
    issue: &Value,
    budget: &mut usize,
) -> Result<Vec<IssueLocation>> {
    if array(issue, "locations")?.len() > 256 {
        bail!("SARIF result exceeds 256 primary locations");
    }
    let mut result = Vec::new();
    for location in array(issue, "locations")? {
        object(location)?;
        let mut symbols = std::collections::BTreeSet::new();
        for value in array(location, "logicalLocations")? {
            if let Some(symbol) = logical(run, value, 0, budget)? {
                symbols.insert(symbol);
            }
        }
        let symbol = if symbols.is_empty() {
            None
        } else if symbols.len() == 1 {
            symbols.into_iter().next()
        } else {
            Some(serde_json::to_string(&symbols)?)
        };
        let (mut path, mut line, mut end_line) = (None, None, None);
        if let Some(physical) = location.get("physicalLocation") {
            object(physical)?;
            if physical.get("address").is_some() {
                bail!("SARIF binary addresses require an address-to-source adapter");
            }
            if let Some(artifact) = physical.get("artifactLocation") {
                path = Some(file(run, artifact, budget)?);
            }
            if let Some(region) = physical.get("region") {
                object(region)?;
                for name in ["startLine", "endLine", "startColumn", "endColumn"] {
                    if let Some(value) = region.get(name)
                        && value.as_u64().is_none_or(|value| value == 0)
                    {
                        bail!("SARIF region coordinates must be positive integers");
                    }
                }
                line = region
                    .get("startLine")
                    .map(|value| usize::try_from(value.as_u64().unwrap()))
                    .transpose()?;
                end_line = region
                    .get("endLine")
                    .map(|value| usize::try_from(value.as_u64().unwrap()))
                    .transpose()?;
                if end_line.is_some_and(|end| line.is_none_or(|start| end < start)) {
                    bail!("SARIF region ends before its start");
                }
            }
        }
        if path.is_none() && symbol.is_none() {
            bail!("SARIF location has neither a source file nor a logical symbol");
        }
        let location = IssueLocation {
            file: path,
            line,
            end_line,
            symbol,
        };
        if !result.contains(&location) {
            result.push(location);
        }
    }
    Ok(result)
}

pub(super) fn rule<'a>(
    run: &'a Value,
    issue: &'a Value,
    lookup: &std::collections::BTreeMap<&str, &'a Value>,
) -> Result<(String, Option<&'a Value>)> {
    let reference = issue.get("rule");
    if let Some(reference) = reference {
        object(reference)?;
        if reference.get("toolComponent").is_some() {
            bail!("SARIF extension rule references need an extension adapter");
        }
    }
    let outer_id = optional_text(issue, "ruleId")?;
    let inner_id = reference
        .map(|value| optional_text(value, "id"))
        .transpose()?
        .flatten();
    if outer_id.is_some() && inner_id.is_some() && outer_id != inner_id {
        bail!("SARIF rule ID references disagree");
    }
    let outer_index = index(issue, "ruleIndex")?;
    let inner_index = reference
        .map(|value| index(value, "index"))
        .transpose()?
        .flatten();
    if outer_index.is_some() && inner_index.is_some() && outer_index != inner_index {
        bail!("SARIF rule index references disagree");
    }
    let rules = array(&run["tool"]["driver"], "rules")?;
    let descriptor = outer_index
        .or(inner_index)
        .map(|index| {
            rules
                .get(index)
                .context("SARIF rule index is out of bounds")
        })
        .transpose()?;
    let descriptor_id = descriptor.map(|value| text(value, "id")).transpose()?;
    let id = outer_id
        .or(inner_id)
        .or(descriptor_id)
        .context("SARIF result has no rule identity")?;
    if descriptor_id.is_some_and(|value| value != id) {
        bail!("SARIF rule index contradicts its ID");
    }
    Ok((id.into(), descriptor.or_else(|| lookup.get(id).copied())))
}
