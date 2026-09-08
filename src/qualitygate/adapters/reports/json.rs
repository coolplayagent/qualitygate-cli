use super::*;
use anyhow::{Context, Result, bail};
use serde_json::Value;

pub(super) fn sarif(text: &str) -> Result<Data> {
    let document: Value = serde_json::from_str(text)?;
    if document["version"] != "2.1.0" {
        bail!("Only SARIF 2.1.0 is supported");
    }
    let runs = document["runs"].as_array().context("SARIF runs missing")?;
    if runs.is_empty() {
        bail!("SARIF contains no analysis runs");
    }
    let mut data = Data::default();
    for run in runs {
        if run["invocations"].as_array().is_some_and(|items| {
            items
                .iter()
                .any(|item| item["executionSuccessful"] == false)
        }) {
            bail!("SARIF analysis did not execute successfully");
        }
        for issue in run["results"].as_array().context("SARIF results missing")? {
            let location = &issue["locations"][0]["physicalLocation"];
            let line = location["region"]["startLine"]
                .as_u64()
                .map(usize::try_from)
                .transpose()?;
            data.issues.push(Issue {
                rule: issue["ruleId"]
                    .as_str()
                    .unwrap_or("sarif-diagnostic")
                    .into(),
                file: location["artifactLocation"]["uri"].as_str().map(Into::into),
                line,
                message: issue["message"]["text"]
                    .as_str()
                    .or_else(|| issue["message"]["markdown"].as_str())
                    .context("SARIF message missing")?
                    .into(),
                symbol: issue["locations"][0]["logicalLocations"][0]["fullyQualifiedName"]
                    .as_str()
                    .map(Into::into),
            });
        }
    }
    Ok(data)
}
