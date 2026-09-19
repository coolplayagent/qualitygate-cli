//! Skill-owned built-in rule asset discovery, isolated from policy snapshots.

use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
const PACKAGES: &[&str] = &[
    "core",
    "shared",
    "lang-java",
    "lang-python",
    "lang-typescript",
    "lang-go",
    "lang-c",
    "lang-cpp",
];

#[derive(Debug)]
pub(super) struct PackagedRule {
    pub(super) package: String,
    pub(super) path: String,
    pub(super) bytes: Vec<u8>,
}

pub(super) fn packaged_rules() -> Result<Vec<PackagedRule>> {
    let root = builtin_rules_directory()?;
    packaged_rules_from(&root)
}

fn builtin_rules_directory() -> Result<PathBuf> {
    if let Some(path) = crate::env::builtin_rule_assets_override() {
        return canonical_rule_directory(path, crate::env::BUILTIN_RULES_DIR_ENV);
    }
    for candidate in crate::env::builtin_rule_asset_candidates() {
        if candidate.is_dir() {
            return canonical_rule_directory(candidate, "installed Qualitygate skill");
        }
    }
    bail!(
        "Cannot locate built-in rule assets; install the Qualitygate skill beside the executable or set {}",
        crate::env::BUILTIN_RULES_DIR_ENV
    )
}

fn canonical_rule_directory(path: PathBuf, source: &str) -> Result<PathBuf> {
    let path = dunce::canonicalize(&path)
        .with_context(|| format!("Cannot read built-in rule assets from {source}"))?;
    if !std::fs::metadata(&path)?.is_dir() {
        bail!(
            "Built-in rule asset path is not a directory: {}",
            path.display()
        );
    }
    Ok(path)
}

fn packaged_rules_from(root: &Path) -> Result<Vec<PackagedRule>> {
    let mut result = Vec::new();
    let mut total = 0u64;
    for package in PACKAGES {
        let directory = root.join(package);
        let metadata = std::fs::symlink_metadata(&directory).with_context(|| {
            format!("Built-in skill rule package is missing: references/rules/{package}")
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            bail!(
                "Built-in rule package must be a real directory: {}",
                directory.display()
            );
        }
        let mut files = Vec::new();
        for entry in std::fs::read_dir(&directory)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if file_type.is_dir() || !(name.ends_with(".yaml") || name.ends_with(".yml")) {
                bail!("Built-in rules accept only YAML files: {package}/{name}");
            }
            // Bazel materializes declared data as runfile symlinks. Skill
            // installation is the trust boundary for this explicit asset root;
            // require the resolved target to be a bounded regular file.
            let metadata = std::fs::metadata(entry.path())?;
            if !metadata.is_file() {
                bail!("Built-in rules accept only regular files: {package}/{name}");
            }
            total = total.saturating_add(metadata.len());
            if result.len() + files.len() >= 256 || total > super::MAX_CONFIG_BYTES as u64 {
                bail!("Built-in rule assets exceed 256 files or 1 MiB");
            }
            files.push((name, std::fs::read(entry.path())?));
        }
        if files.is_empty() {
            bail!("Built-in rule package is empty: references/rules/{package}");
        }
        files.sort_by(|left, right| left.0.cmp(&right.0));
        result.extend(files.into_iter().map(|(path, bytes)| PackagedRule {
            package: (*package).into(),
            path,
            bytes,
        }));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_skill_package_is_never_treated_as_an_empty_rule_catalog() {
        let root = tempfile::tempdir().unwrap();
        let error = packaged_rules_from(root.path()).unwrap_err().to_string();
        assert!(error.contains("Built-in skill rule package is missing"));
    }
}
