//! Bounded candidate-policy discovery. No repository command is executed here.

mod inventory;
mod projects;
mod suggestions;
#[cfg(test)]
mod tests;

use super::{CommandCheck, Config};
use anyhow::Result;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

#[derive(Debug, Serialize)]
pub struct Discovery {
    pub schema_version: u32,
    pub scope: &'static str,
    pub commands_executed: bool,
    pub files_seen: usize,
    pub entries_seen: usize,
    pub bytes_read: usize,
    pub excluded_directories: &'static [&'static str],
    pub ignore_policy: &'static str,
    pub inputs: BTreeMap<String, String>,
    pub languages: Vec<LanguageDetection>,
    pub projects: Vec<Project>,
    pub available_rulesets: BTreeMap<String, Vec<serde_json::Value>>,
    pub suggested_checks: Vec<Suggestion>,
    pub gaps: Vec<Gap>,
}

#[derive(Debug, Serialize)]
pub struct LanguageDetection {
    pub language: String,
    pub source_files: usize,
    pub syntax_capabilities: Vec<&'static str>,
    pub marker_options: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
pub struct Project {
    pub root: String,
    pub ecosystem: String,
    pub manifest: String,
    pub metadata_status: &'static str,
    pub project_capabilities: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
pub struct Gap {
    pub path: String,
    pub capability: String,
    pub reason: String,
}

#[derive(Debug, Serialize)]
pub struct Suggestion {
    pub purpose: String,
    pub source: String,
    pub status: &'static str,
    pub review: Vec<String>,
    pub check: CommandCheck,
}

pub(super) fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

pub fn discover(root: &Path) -> Result<Discovery> {
    let started = std::time::Instant::now();
    let root = dunce::canonicalize(root)?;
    let inventory = inventory::scan(&root)?;
    let mut result = Discovery {
        schema_version: 1,
        scope: "working_directory_discovery",
        commands_executed: false,
        files_seen: inventory.files.len(),
        entries_seen: inventory.entries,
        bytes_read: inventory.bytes_read,
        excluded_directories: inventory::EXCLUDED,
        ignore_policy: "Repository-local .gitignore files; no global or parent ignore configuration",
        inputs: inventory.digests.clone(),
        languages: Vec::new(),
        projects: Vec::new(),
        available_rulesets: BTreeMap::new(),
        suggested_checks: Vec::new(),
        gaps: Vec::new(),
    };
    let mut languages = BTreeMap::<String, usize>::new();
    for file in &inventory.files {
        if let Some(language) = crate::domain::language::for_path(file) {
            *languages.entry(language.name.into()).or_default() += 1;
        } else if let Some(extension) = Path::new(file).extension().and_then(|part| part.to_str()) {
            let language = match extension {
                "rb" => Some("ruby"),
                "php" => Some("php"),
                "cs" => Some("csharp"),
                "kt" | "kts" => Some("kotlin"),
                "c" | "h" => Some("c"),
                "cc" | "cpp" | "hpp" => Some("cpp"),
                "swift" => Some("swift"),
                _ => None,
            };
            if let Some(language) = language {
                *languages.entry(language.into()).or_default() += 1;
            }
        }
    }
    for (path, text) in &inventory.manifests {
        if started.elapsed().as_secs() >= 30 {
            anyhow::bail!("Repository discovery exceeded its 30-second budget")
        }
        let (ecosystem, language) = inventory::manifest_kind(path).expect("classified manifest");
        languages.entry(language.into()).or_default();
        let module = Path::new(path)
            .parent()
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let mut project = Project {
            root: if module.is_empty() {
                ".".into()
            } else {
                module
            },
            ecosystem: ecosystem.into(),
            manifest: path.clone(),
            metadata_status: "unavailable",
            project_capabilities: Vec::new(),
        };
        match projects::analyze(&inventory, &project, text) {
            Ok((capabilities, suggestions, gaps)) => {
                project.metadata_status = if ["cargo", "maven", "node", "go"].contains(&ecosystem)
                    || (ecosystem == "python"
                        && (path.ends_with("pyproject.toml") || path.ends_with("requirements.txt")))
                {
                    "inspected"
                } else {
                    "detected_only"
                };
                project.project_capabilities = capabilities;
                result.suggested_checks.extend(suggestions);
                result.gaps.extend(gaps);
            }
            Err(error) => {
                project.metadata_status = "invalid_or_unsupported";
                result.gaps.push(Gap {
                    path: path.clone(),
                    capability: "manifest_metadata".into(),
                    reason: format!("Cannot derive project suggestions: {error:#}"),
                });
            }
        }
        result.projects.push(project);
    }
    for (name, source_files) in languages {
        let descriptor = crate::domain::language::named(&name);
        if descriptor.is_none() {
            result.gaps.push(Gap {
                path: ".".into(),
                capability: "syntax".into(),
                reason: format!("No syntax adapter for {name}"),
            });
        }
        result.languages.push(LanguageDetection {
            language: name,
            source_files,
            syntax_capabilities: descriptor
                .map_or_else(Vec::new, |value| value.syntax_capabilities.to_vec()),
            marker_options: descriptor.map_or_else(Vec::new, |value| value.marker_options.to_vec()),
        });
    }
    let relevant: BTreeSet<_> = result
        .languages
        .iter()
        .map(|language| language.language.as_str())
        .collect();
    let config = Config {
        rulesets: super::RULESETS.iter().map(|name| (*name).into()).collect(),
        ..Config::default()
    };
    let catalog = super::catalog::Catalog::load(&config, std::iter::empty())?;
    for (id, entry) in catalog.entries {
        let builtin = entry.builtin.expect("embedded catalog");
        if builtin.language.is_empty()
            || builtin
                .language
                .iter()
                .any(|name| relevant.contains(name.as_str()))
        {
            result.available_rulesets.entry(entry.package).or_default().push(serde_json::json!({
                "id": id, "version": builtin.version, "requires_capabilities": builtin.requires_capabilities,
            }));
        }
    }
    result.gaps.push(Gap {
        path: ".".into(), capability: "external_provenance".into(),
        reason: "Marker options describe declarations only; AI-only scope needs signed external records, and git_trailer bindings need complete local Git ancestry".into(),
    });
    if result.projects.is_empty() {
        result.gaps.push(Gap { path: ".".into(), capability: "project_verification".into(), reason: "No recognized project manifest; configure existing build/test commands and their acceptance reports".into() });
    }
    // Suggestion IDs include manifest identity, making mixed/nested roots stable.
    let mut commands = BTreeSet::new();
    result.suggested_checks.retain(|suggestion| {
        commands.insert((
            suggestion.purpose.clone(),
            suggestion.check.cwd.clone(),
            suggestion.check.argv.clone(),
        ))
    });
    result
        .suggested_checks
        .sort_by(|a, b| a.check.id.cmp(&b.check.id));
    if result.suggested_checks.len() > 2048 {
        anyhow::bail!("Discovery exceeds command suggestion budget")
    }
    if started.elapsed().as_secs() >= 30 {
        anyhow::bail!("Repository discovery exceeded its 30-second budget")
    }
    Ok(result)
}
