//! Confined project-rule discovery and bounded parallel parsing of immutable bytes.

use super::{
    Config, CustomRule,
    catalog::{Entry, project_rules_directory},
    parallel,
};
use anyhow::{Context, Result, bail};
use std::{collections::BTreeMap, path::Path};

pub(super) fn read(root: &Path, config: &Config) -> Result<BTreeMap<String, Vec<u8>>> {
    let Some(requested) = project_rules_directory(config) else {
        return Ok(BTreeMap::new());
    };
    let deadline = parallel::deadline();
    let directory = crate::paths::confined(root, Path::new(requested))?;
    if !std::fs::metadata(&directory)
        .with_context(|| format!("Configured project rule directory does not exist: {requested}"))?
        .is_dir()
    {
        bail!("Project rule path is not a directory: {requested}");
    }
    let mut pending = vec![directory];
    let mut entries = 0;
    let mut total = 0;
    let mut files = Vec::new();
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory)? {
            if std::time::Instant::now() >= deadline {
                bail!("Rule loading exceeded its deadline");
            }
            let entry = entry?;
            entries += 1;
            if entries > 4096 {
                bail!("Project rule directory exceeds discovery budget");
            }
            let relative = crate::paths::from_native(entry.path().strip_prefix(root)?)?;
            let path = crate::paths::confined(root, relative.as_ref())?;
            let metadata = std::fs::metadata(&path)?;
            if metadata.is_dir() {
                pending.push(path);
            } else if path
                .extension()
                .is_some_and(|extension| extension == "yaml" || extension == "yml")
            {
                if !metadata.is_file() {
                    bail!("Rule inputs must be regular files: {relative}");
                }
                if files.len() >= 256 {
                    bail!("Project rule package exceeds 256 files");
                }
                total += metadata.len();
                if total > super::MAX_CONFIG_BYTES as u64 {
                    bail!("Project rules exceed 1 MiB");
                }
                files.push((relative, metadata.len()));
            }
        }
    }
    if files.is_empty() {
        bail!("Configured project rule directory contains no YAML definitions: {requested}");
    }
    files.sort();
    Ok(
        parallel::map(&files, parallel::jobs(), deadline, |(path, length)| {
            let bytes = super::rule_authoring::read_file(root, path)?;
            if bytes.len() as u64 != *length {
                bail!("Project rule changed during discovery: {path}");
            }
            Ok((path.clone(), bytes))
        })?
        .into_iter()
        .collect(),
    )
}

pub(super) fn parse<'a>(
    config: &Config,
    files: impl IntoIterator<Item = (&'a str, &'a [u8])>,
    jobs: usize,
) -> Result<BTreeMap<String, Entry>> {
    let Some(directory) = project_rules_directory(config) else {
        return Ok(BTreeMap::new());
    };
    let prefix = format!("{}/", directory.trim_end_matches('/'));
    let mut selected = Vec::new();
    let mut total = 0;
    for (path, bytes) in files {
        if !path.starts_with(&prefix) || !(path.ends_with(".yaml") || path.ends_with(".yml")) {
            continue;
        }
        total += bytes.len();
        if total > super::MAX_CONFIG_BYTES || selected.len() >= 256 {
            bail!("Custom rule package exceeds 256 files or 1 MiB");
        }
        selected.push((path, bytes));
    }
    if selected.is_empty() {
        bail!("Configured project rule directory contains no YAML definitions: {directory}");
    }
    selected.sort_by_key(|(path, _)| *path);
    let parsed = parallel::map(&selected, jobs, parallel::deadline(), |(path, bytes)| {
        let rule: CustomRule = super::rule_schema::parse(bytes)
            .with_context(|| format!("Invalid custom rule: {path}"))?;
        Ok((
            rule.id.clone(),
            Entry {
                origin: (*path).into(),
                package: "custom".into(),
                builtin: None,
                custom: Some(rule),
            },
        ))
    })?;
    let mut entries = BTreeMap::new();
    for (id, entry) in parsed {
        if entries.insert(id.clone(), entry).is_some() {
            bail!("Duplicate custom rule id: {id}");
        }
    }
    Ok(entries)
}

pub(super) fn discover(root: &Path, config: &Config) -> Result<BTreeMap<String, Entry>> {
    let files = read(root, config)?;
    parse(
        config,
        files
            .iter()
            .map(|(path, bytes)| (path.as_str(), bytes.as_slice())),
        parallel::jobs(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::{Duration, Instant};

    fn rule(index: usize) -> Vec<u8> {
        serde_json::to_vec(&json!({"id":format!("rule-{index:03}"),"version":1,
            "source":{"document":"AGENTS.md","section":"Rules","content_hash":format!("sha256:{}", "a".repeat(64))},
            "requires_capabilities":["files"],"when":{"entity":"file"},"then":{"forbid_pattern":"danger"},"fix":"Review the matched source"})).unwrap()
    }

    #[test]
    fn full_rule_package_parallel_parsing_matches_serial_and_has_a_time_budget() {
        let config = Config {
            custom_rules: Some("qualitygate/rules".into()),
            ..Config::default()
        };
        let mut files: Vec<_> = (0..256)
            .map(|i| (format!("qualitygate/rules/rule-{i:03}.yaml"), rule(i)))
            .collect();
        // Warm only the shared compiled schema, not the rule files themselves.
        super::super::rule_schema::document().unwrap();
        let mut expected = None;
        for jobs in [1, 4, 4, 1] {
            files.reverse();
            let start = Instant::now();
            let observed = parse(
                &config,
                files
                    .iter()
                    .map(|(path, bytes)| (path.as_str(), bytes.as_slice())),
                jobs,
            )
            .unwrap();
            let elapsed = start.elapsed();
            eprintln!("issue3 rule parsing: files=256 jobs={jobs} elapsed={elapsed:?}");
            assert!(
                elapsed < Duration::from_secs(5),
                "Rule parsing exceeded 5-second regression budget: {elapsed:?}"
            );
            assert_eq!(observed.len(), 256);
            let observed = serde_json::to_value(observed).unwrap();
            if let Some(expected) = &expected {
                assert_eq!(expected, &observed);
            } else {
                expected = Some(observed);
            }
        }
        files[0].1 = b"invalid rule".to_vec();
        let serial = parse(
            &config,
            files
                .iter()
                .map(|(path, bytes)| (path.as_str(), bytes.as_slice())),
            1,
        )
        .unwrap_err();
        let parallel = parse(
            &config,
            files
                .iter()
                .map(|(path, bytes)| (path.as_str(), bytes.as_slice())),
            4,
        )
        .unwrap_err();
        assert_eq!(format!("{serial:#}"), format!("{parallel:#}"));
        files[0].1 = rule(0);
        files.push(("qualitygate/rules/excess.yaml".into(), rule(257)));
        assert!(
            parse(
                &config,
                files
                    .iter()
                    .map(|(path, bytes)| (path.as_str(), bytes.as_slice())),
                4
            )
            .is_err()
        );
        assert!(
            parse(
                &config,
                [(
                    "qualitygate/rules/large.yaml",
                    vec![b' '; super::super::MAX_CONFIG_BYTES + 1].as_slice()
                )],
                4
            )
            .is_err()
        );
        assert!(parse(&config, [("elsewhere.yaml", b"ignored".as_slice())], 4).is_err());
    }
}
