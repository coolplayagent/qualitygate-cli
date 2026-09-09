//! Snapshot declarations, pip resolution, and installed Core Metadata must agree.

use super::python_metadata::{self, Metadata};
use crate::{
    config::PythonProject,
    domain::*,
    snapshot::{self, Snapshot},
};
use anyhow::{Context, Result, bail};
use pep508_rs::{
    ExtraName, MarkerEnvironment, MarkerTree, PackageName, Requirement, VersionOrUrl,
    pep440_rs::{Version, VersionSpecifiers},
};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    time::{Duration, Instant},
};

#[cfg(test)]
#[path = "python_tests.rs"]
mod tests;

type Req = Requirement<reqwest::Url>;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InstallReport {
    version: String,
    pip_version: String,
    environment: BTreeMap<String, String>,
    install: Vec<Installed>,
}

#[derive(Deserialize)]
struct Installed {
    download_info: serde_json::Value,
    is_direct: bool,
    requested: bool,
    #[serde(default)]
    requested_extras: Vec<String>,
    metadata: Metadata,
}

pub fn parse(
    spec: &PythonProject,
    bytes: &[u8],
    installed: &[(String, Vec<u8>)],
    workspace: &Path,
    snapshot: &Snapshot,
    producer: &str,
    versions: &BTreeMap<String, String>,
) -> Result<ProjectFacts> {
    if bytes.len() > snapshot::MAX_FILE_BYTES || installed.len() > 4096 {
        bail!("Python report exceeds analysis budget");
    }
    let started = Instant::now();
    let report: InstallReport = serde_json::from_slice(bytes)?;
    if report.version != "1" || report.install.is_empty() || report.install.len() > 4096 {
        bail!("Unsupported or empty pip installation report");
    }
    if !versions
        .get("pip")
        .is_some_and(|version| version.starts_with(&format!("pip {} ", report.pip_version)))
    {
        bail!("pip report and actual version probe disagree");
    }
    let environment: MarkerEnvironment =
        serde_json::from_value(serde_json::to_value(&report.environment)?)?;
    if versions.get("python").map(String::as_str)
        != Some(format!("Python {}", report.environment["python_full_version"]).as_str())
    {
        bail!("Python report and installation interpreter versions disagree");
    }
    let python_version: Version = report.environment["python_full_version"].parse()?;
    let short = python_version
        .release()
        .iter()
        .take(2)
        .map(u64::to_string)
        .collect::<Vec<_>>()
        .join(".");
    if report.environment["python_version"] != short {
        bail!("pip marker environment has inconsistent Python versions");
    }
    let manifest = if spec.root == "." {
        "pyproject.toml".into()
    } else {
        format!("{}/pyproject.toml", spec.root)
    };
    let input = snapshot
        .files
        .get(&manifest)
        .context("Python manifest is absent from snapshot")?;
    let model: toml::Value = toml::from_str(std::str::from_utf8(&input.bytes)?)?;
    let project = model
        .get("project")
        .and_then(toml::Value::as_table)
        .context("Python facts require a PEP 621 project table")?;
    if project
        .get("dynamic")
        .is_some_and(|value| value.as_array().is_none_or(|values| !values.is_empty()))
    {
        bail!("Dynamic Python project metadata is not yet supported");
    }
    let name: PackageName = project
        .get("name")
        .and_then(toml::Value::as_str)
        .context("Missing Python project name")?
        .parse()?;
    let version: Version = project
        .get("version")
        .and_then(toml::Value::as_str)
        .context("Missing static Python project version")?
        .parse()?;
    requires_python(
        project
            .get("requires-python")
            .map(|value| value.as_str().context("requires-python must be a string"))
            .transpose()?,
        &python_version,
    )?;
    let extras = spec
        .extras
        .iter()
        .map(|extra| extra.parse())
        .collect::<std::result::Result<Vec<ExtraName>, _>>()?;
    let mut declared_requirements = all_requirements(&strings(project.get("dependencies"))?)?;
    let mut optional = BTreeMap::new();
    if let Some(value) = project.get("optional-dependencies") {
        for (extra, requirements) in value
            .as_table()
            .context("optional-dependencies must be a table")?
        {
            let extra: ExtraName = extra.parse()?;
            if optional
                .insert(extra, strings(Some(requirements))?)
                .is_some()
            {
                bail!("Ambiguous normalized Python extra");
            }
        }
    }
    for extra in &extras {
        for requirement in all_requirements(
            optional
                .get(extra)
                .context("Selected Python extra is not declared")?,
        )? {
            declared_requirements.insert(requirement.with_extra_marker(extra));
        }
    }
    let declared = active_requirements(declared_requirements, &environment, &extras)?;
    let mut packages = BTreeMap::new();
    for package in &report.install {
        let name: PackageName = package.metadata.name.parse()?;
        if packages.insert(name, package).is_some() {
            bail!("Duplicate normalized distribution in pip report");
        }
        requires_python(package.metadata.requires_python.as_deref(), &python_version)?;
    }
    let root = *packages
        .get(&name)
        .context("pip report omits the snapshot project")?;
    if root.metadata.version.parse::<Version>()? != version || !root.requested || !root.is_direct {
        bail!("pip report project identity does not match the manifest");
    }
    let url = reqwest::Url::parse(
        root.download_info
            .get("url")
            .and_then(serde_json::Value::as_str)
            .context("Missing Python project origin")?,
    )?;
    let expected = crate::paths::confined(workspace, Path::new(&spec.root))?;
    if url.to_file_path().ok().as_ref() != Some(&expected)
        || url.fragment().is_some()
        || url.query().is_some()
        || root
            .download_info
            .get("dir_info")
            .and_then(serde_json::Value::as_object)
            .is_none_or(|info| !info.is_empty())
    {
        bail!("Python project origin is not the checked local non-editable directory");
    }
    let reported_extras = root
        .requested_extras
        .iter()
        .map(|extra| extra.parse())
        .collect::<std::result::Result<BTreeSet<ExtraName>, _>>()?;
    if reported_extras != extras.iter().cloned().collect() {
        bail!("pip report requested extras differ from policy");
    }
    if declared != requirements(root.metadata.requires_dist.clone(), &environment, &extras)? {
        bail!("Built Python requirements differ from snapshot declarations");
    }
    let mut actual = BTreeMap::new();
    let mut metadata_digests = BTreeMap::new();
    let mut total = 0;
    for (_, bytes) in installed {
        total += bytes.len();
        if bytes.len() > snapshot::MAX_FILE_BYTES || total > snapshot::MAX_SNAPSHOT_BYTES {
            bail!("Installed Python metadata exceeds evidence budget");
        }
        let metadata = python_metadata::parse(bytes)?;
        let key: PackageName = metadata.name.parse()?;
        metadata_digests.insert(key.to_string(), snapshot::digest(bytes));
        if actual.insert(key, metadata).is_some() {
            bail!("Duplicate installed Python distribution");
        }
    }
    if packages.keys().ne(actual.keys()) {
        bail!("Installed distributions differ from pip report");
    }
    for (name, package) in &packages {
        let installed = &actual[name];
        if package.metadata.version.parse::<Version>()? != installed.version.parse::<Version>()?
            || all_requirements(&package.metadata.requires_dist)?
                != all_requirements(&installed.requires_dist)?
            || package.metadata.requires_python != installed.requires_python
            || package
                .metadata
                .provides_extra
                .iter()
                .collect::<BTreeSet<_>>()
                != installed.provides_extra.iter().collect()
        {
            bail!("Installed Core Metadata disagrees with pip report for {name}");
        }
        if name != &root.metadata.name.parse::<PackageName>()? && package.requested {
            bail!("pip report contains another requested project");
        }
    }
    // Revisit a node when another edge activates more extras. Missing transitive
    // nodes or conflicting versions must not disappear behind --no-deps behavior.
    let mut activated: BTreeMap<PackageName, BTreeSet<ExtraName>> = BTreeMap::new();
    activated.insert(name.clone(), extras.iter().cloned().collect());
    let mut pending = vec![name.clone()];
    let mut steps = 0;
    while let Some(name) = pending.pop() {
        steps += 1;
        if steps > 50_000 || started.elapsed() > Duration::from_secs(30) {
            bail!("Python resolution exceeds analysis budget");
        }
        let package = packages[&name];
        let enabled: Vec<_> = activated[&name].iter().cloned().collect();
        let offered = package
            .metadata
            .provides_extra
            .iter()
            .map(|extra| extra.parse())
            .collect::<std::result::Result<BTreeSet<ExtraName>, _>>()?;
        if enabled.iter().any(|extra| !offered.contains(extra)) {
            bail!("Requested Python dependency extra is unavailable for {name}");
        }
        for requirement in requirements(
            package.metadata.requires_dist.clone(),
            &environment,
            &enabled,
        )? {
            let target = packages.get(&requirement.name).context(format!(
                "Missing resolved Python dependency: {}",
                requirement.name
            ))?;
            if let Some(VersionOrUrl::VersionSpecifier(specifier)) = &requirement.version_or_url
                && !specifier.contains(&target.metadata.version.parse()?)
            {
                bail!("Resolved Python version violates requirement {requirement}");
            }
            let first = !activated.contains_key(&requirement.name);
            let enabled = activated.entry(requirement.name.clone()).or_default();
            let previous = enabled.len();
            enabled.extend(requirement.extras);
            if enabled.len() > 64 {
                bail!("Python dependency extras exceed analysis budget");
            }
            if first || enabled.len() != previous {
                pending.push(requirement.name);
            }
        }
    }
    if activated.keys().ne(packages.keys()) {
        bail!("pip report contains distributions outside the requested dependency closure");
    }
    let fact = |name: &PackageName| DependencyFact {
        group: "pypi".into(),
        artifact: name.to_string(),
        version: packages[name].metadata.version.clone(),
        artifact_type: "distribution".into(),
        classifier: String::new(),
        scope: "environment".into(),
    };
    Ok(ProjectFacts {
        schema_version: 1,
        ecosystem: "python".into(),
        root: spec.root.clone(),
        manifest,
        coordinate: format!("pypi:{name}:{version}"),
        source_root: spec.source_root.clone(),
        test_source_root: spec.test_source_root.clone(),
        declared: declared
            .iter()
            .map(|requirement| fact(&requirement.name))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        resolved: packages
            .keys()
            .filter(|key| **key != name)
            .map(fact)
            .collect(),
        dependency_usage: None,
        python: Some(PythonResolution {
            pip_version: report.pip_version,
            environment: report.environment,
            extras: spec.extras.clone(),
            install_target: spec.install_target.clone(),
            installed_metadata: metadata_digests,
        }),
        producer_check: producer.into(),
        snapshot_digest: snapshot.identity.content_digest.clone(),
    })
}

fn strings(value: Option<&toml::Value>) -> Result<Vec<String>> {
    value
        .map(|value| {
            value
                .as_array()
                .context("Python dependencies must be an array")?
                .iter()
                .map(|value| {
                    Ok(value
                        .as_str()
                        .context("Python requirement must be a string")?
                        .into())
                })
                .collect()
        })
        .unwrap_or_else(|| Ok(vec![]))
}

fn all_requirements(values: &[String]) -> Result<BTreeSet<Req>> {
    let mut result = BTreeSet::new();
    for value in values {
        let mut depth = 0usize;
        if value.len() > 8192 {
            bail!("Python requirement exceeds parsing budget");
        }
        for character in value.chars() {
            if character == '(' {
                depth += 1;
            }
            if depth > 64 {
                bail!("Python requirement nesting exceeds parsing budget");
            }
            if character == ')' {
                depth = depth.saturating_sub(1);
            }
        }
        let mut warnings = Vec::new();
        let mut requirement =
            Req::parse_reporter(value, ".", &mut |_, message| warnings.push(message))?;
        if !warnings.is_empty() || matches!(requirement.version_or_url, Some(VersionOrUrl::Url(_)))
        {
            bail!("Unsupported or ambiguous Python dependency requirement: {value}");
        }
        requirement.extras.sort();
        requirement.extras.dedup();
        result.insert(requirement);
    }
    Ok(result)
}

fn requirements(
    values: Vec<String>,
    env: &MarkerEnvironment,
    extras: &[ExtraName],
) -> Result<BTreeSet<Req>> {
    active_requirements(all_requirements(&values)?, env, extras)
}

fn active_requirements(
    values: BTreeSet<Req>,
    env: &MarkerEnvironment,
    extras: &[ExtraName],
) -> Result<BTreeSet<Req>> {
    let mut result = BTreeSet::new();
    for mut requirement in values {
        // String equality is valid PEP 508; the library's ordered decision tree
        // can emit a lexicographic warning while evaluating that equality.
        let (active, warnings) = requirement.evaluate_markers_and_report(env, extras);
        if warnings
            .iter()
            .any(|(kind, _)| !matches!(kind, pep508_rs::MarkerWarningKind::LexicographicComparison))
        {
            bail!("Ambiguous Python environment marker: {warnings:?}");
        }
        if active {
            requirement.marker = MarkerTree::TRUE;
            result.insert(requirement);
        }
    }
    Ok(result)
}

fn requires_python(specifier: Option<&str>, version: &Version) -> Result<()> {
    if let Some(specifier) = specifier
        && !specifier.parse::<VersionSpecifiers>()?.contains(version)
    {
        bail!("Installed distribution does not support the reported Python interpreter");
    }
    Ok(())
}
