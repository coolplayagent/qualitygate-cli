//! Normalize the supported self-contained SARIF 2.1.0 profile.

use super::*;
use anyhow::{Context, Result, bail};
use serde_json::{Map, Value};

pub(super) fn object(value: &Value) -> Result<&Map<String, Value>> {
    value
        .as_object()
        .context("SARIF object has an invalid type")
}

pub(super) fn array<'a>(value: &'a Value, key: &str) -> Result<&'a [Value]> {
    match value.get(key) {
        None => Ok(&[]),
        Some(value) => value
            .as_array()
            .map(Vec::as_slice)
            .with_context(|| format!("SARIF {key} must be an array")),
    }
}

pub(super) fn optional_text<'a>(value: &'a Value, key: &str) -> Result<Option<&'a str>> {
    match value.get(key) {
        None => Ok(None),
        Some(value) => {
            let text = value
                .as_str()
                .with_context(|| format!("SARIF {key} must be a string"))?;
            if text.trim().is_empty() || text.contains('\0') {
                bail!("SARIF {key} must be nonempty text");
            }
            Ok(Some(text))
        }
    }
}

pub(super) fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    optional_text(value, key)?.with_context(|| format!("SARIF {key} is missing"))
}

fn invocations(run: &Value) -> Result<()> {
    for invocation in array(run, "invocations")? {
        object(invocation)?;
        if invocation
            .get("executionSuccessful")
            .and_then(Value::as_bool)
            != Some(true)
        {
            bail!("SARIF invocation lacks successful execution evidence");
        }
        for kind in [
            "toolExecutionNotifications",
            "toolConfigurationNotifications",
        ] {
            for notification in array(invocation, kind)? {
                object(notification)?;
                let level = optional_text(notification, "level")?.unwrap_or("warning");
                if !["none", "note", "warning", "error"].contains(&level) {
                    bail!("Invalid SARIF notification level");
                }
                if level == "error" || notification.get("exception").is_some() {
                    bail!("SARIF analyzer reported an execution/configuration error");
                }
            }
        }
    }
    if let Some(references) = run.get("externalPropertyFileReferences")
        && !object(references)?.is_empty()
    {
        bail!("External SARIF properties are not a complete self-contained report");
    }
    if run.get("baselineGuid").is_some() {
        bail!(
            "Pre-baselined SARIF needs its original snapshot binding; generate fresh results for qualitygate comparison"
        );
    }
    Ok(())
}

fn message(run: &Value, issue: &Value, rule: Option<&Value>) -> Result<String> {
    let message = issue
        .get("message")
        .context("SARIF result message is missing")?;
    object(message)?;
    let mut template = optional_text(message, "text")?;
    if template.is_none() {
        let id = text(message, "id")?;
        let descriptor = rule
            .and_then(|rule| rule.get("messageStrings"))
            .and_then(|messages| messages.get(id));
        let global = run["tool"]["driver"]
            .get("globalMessageStrings")
            .and_then(|messages| messages.get(id));
        let found = descriptor
            .or(global)
            .context("Unresolved SARIF message ID")?;
        template = optional_text(found, "text")?;
    }
    let template = template.context("SARIF message has no supported text")?;
    let arguments: Vec<_> = array(message, "arguments")?
        .iter()
        .map(|value| {
            value
                .as_str()
                .context("SARIF message arguments must be strings")
        })
        .collect::<Result<_>>()?;
    let mut output = String::new();
    let mut chars = template.chars().peekable();
    while let Some(character) = chars.next() {
        if (character == '{' || character == '}') && chars.peek() == Some(&character) {
            chars.next();
            output.push(character);
        } else if character == '{' {
            let mut number = String::new();
            while chars.peek().is_some_and(char::is_ascii_digit) {
                number.push(chars.next().unwrap());
            }
            if number.is_empty() || chars.next() != Some('}') {
                bail!("Unsupported SARIF message placeholder");
            }
            let index = number.parse::<usize>()?;
            output.push_str(
                arguments
                    .get(index)
                    .context("SARIF message argument is missing")?,
            );
        } else if character == '}' {
            bail!("Unescaped SARIF message brace");
        } else {
            output.push(character);
        }
        if output.len() > crate::snapshot::MAX_FILE_BYTES {
            bail!("Expanded SARIF message exceeds its byte budget");
        }
    }
    Ok(output)
}

pub(super) fn sarif(text: &str) -> Result<Data> {
    let document: Value = serde_json::from_str(text)?;
    object(&document)?;
    if document["version"] != "2.1.0" {
        bail!("Only SARIF 2.1.0 is supported");
    }
    if !array(&document, "inlineExternalProperties")?.is_empty() {
        bail!("Inline external SARIF properties require explicit resolution");
    }
    let runs = array(&document, "runs")?;
    if runs.is_empty() {
        bail!("SARIF contains no analysis runs");
    }
    let mut data = Data::default();
    let mut expanded_bytes = 0;
    let mut location_budget = 200_000;
    for run in runs {
        object(run)?;
        let driver = &run["tool"]["driver"];
        object(driver)?;
        let tool = self::text(driver, "name")?;
        let version =
            optional_text(driver, "semanticVersion")?.or(optional_text(driver, "version")?);
        invocations(run)?;
        if !run.get("results").is_some_and(Value::is_array) {
            bail!("SARIF requires an explicit results array from a completed run");
        }
        let results = array(run, "results")?;
        if results.len() > 50_000 {
            bail!("SARIF run exceeds 50000 results");
        }
        let mut lookup = std::collections::BTreeMap::new();
        for rule in array(driver, "rules")? {
            object(rule)?;
            if lookup.insert(self::text(rule, "id")?, rule).is_some() {
                bail!("Duplicate SARIF rule IDs");
            }
        }
        let mut summary = SarifRun {
            tool: tool.into(),
            version: version.map(str::to_owned),
            results: results.len(),
            violations: 0,
            non_violations: 0,
            suppressed_results: 0,
        };
        for issue in results {
            object(issue)?;
            if issue.get("baselineState").is_some() {
                bail!(
                    "SARIF result baselineState needs an independently bound baseline; use fresh results"
                );
            }
            let kind = optional_text(issue, "kind")?.unwrap_or("fail");
            let level = optional_text(issue, "level")?;
            if level.is_some_and(|value| !["none", "note", "warning", "error"].contains(&value)) {
                bail!("Invalid SARIF result level");
            }
            if kind != "fail" && level.is_some_and(|value| value != "none") {
                bail!("Non-failing SARIF results cannot carry violation severity");
            }
            let (rule, descriptor) = super::sarif_locations::rule(run, issue, &lookup)?;
            let message = message(run, issue, descriptor)?;
            let locations = super::sarif_locations::locations(run, issue, &mut location_budget)?;
            let first = locations.first();
            let normalized = Issue {
                rule,
                file: first.and_then(|location| location.file.clone()),
                line: first.and_then(|location| location.line),
                symbol: first.and_then(|location| location.symbol.clone()),
                message,
                tool: Some(tool.into()),
                locations,
            };
            expanded_bytes += serde_json::to_vec(&normalized)?.len();
            if expanded_bytes > 16 * 1024 * 1024 {
                bail!("Expanded SARIF results exceed 16 MiB");
            }
            match kind {
                "fail" => {}
                "pass" | "notApplicable" | "informational" => {
                    summary.non_violations += 1;
                    continue;
                }
                "open" | "review" => bail!("SARIF analysis contains an unresolved {kind} result"),
                _ => bail!("Unsupported SARIF result kind"),
            }
            let suppressions = array(issue, "suppressions")?;
            for suppression in suppressions {
                object(suppression)?;
                if !["inSource", "external"].contains(&self::text(suppression, "kind")?) {
                    bail!("Invalid SARIF suppression kind");
                }
                if optional_text(suppression, "status")?
                    .is_some_and(|value| !["accepted", "underReview", "rejected"].contains(&value))
                {
                    bail!("Invalid SARIF suppression status");
                }
            }
            summary.suppressed_results += usize::from(!suppressions.is_empty());
            summary.violations += 1;
            data.issues.push(normalized);
        }
        data.sarif_runs.push(summary);
    }
    Ok(data)
}

#[cfg(test)]
#[path = "sarif_tests.rs"]
mod tests;
