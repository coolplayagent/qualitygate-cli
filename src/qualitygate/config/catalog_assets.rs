//! Skill-owned built-in rule asset discovery, isolated from policy snapshots.

use anyhow::{Context, Result, bail};
use std::{
    io::Read,
    path::{Path, PathBuf},
};
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
    let mut total = 0;
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
            if result.len() + files.len() >= 256 {
                bail!("Built-in rule assets exceed 256 files or 1 MiB");
            }
            let bytes = read_asset(
                &entry.path(),
                &format!("{package}/{name}"),
                super::MAX_CONFIG_BYTES - total,
            )?;
            total += bytes.len();
            files.push((name, bytes));
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

fn read_asset(path: &Path, name: &str, remaining: usize) -> Result<Vec<u8>> {
    // Bazel materializes declared data as runfile symlinks. Skill installation
    // is the trust boundary for this root; resolved targets must be regular.
    let validate = |metadata: std::fs::Metadata| -> Result<()> {
        if !metadata.is_file() {
            bail!("Built-in rules accept only regular files: {name}");
        }
        if metadata.len() > remaining as u64 {
            bail!("Built-in rule assets exceed 256 files or 1 MiB");
        }
        Ok(())
    };
    validate(std::fs::metadata(path)?)?;
    let file = std::fs::File::open(path)?;
    validate(file.metadata()?)?;
    let mut bytes = Vec::new();
    file.take(remaining as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > remaining {
        bail!("Built-in rule assets exceed 256 files or 1 MiB");
    }
    Ok(bytes)
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

    #[test]
    fn asset_byte_and_file_limits_are_inclusive() {
        let root = tempfile::tempdir().unwrap();
        for package in PACKAGES {
            std::fs::create_dir(root.path().join(package)).unwrap();
            std::fs::write(root.path().join(package).join("rule.yaml"), b"{}\n").unwrap();
        }
        let large = root.path().join("core/large.yaml");
        let remaining = super::super::MAX_CONFIG_BYTES - PACKAGES.len() * 3;
        std::fs::write(&large, vec![b' '; remaining]).unwrap();
        assert_eq!(
            packaged_rules_from(root.path())
                .unwrap()
                .iter()
                .map(|rule| rule.bytes.len())
                .sum::<usize>(),
            super::super::MAX_CONFIG_BYTES
        );
        std::fs::write(&large, vec![b' '; remaining + 1]).unwrap();
        assert!(
            packaged_rules_from(root.path())
                .unwrap_err()
                .to_string()
                .contains("1 MiB")
        );
        std::fs::remove_file(large).unwrap();
        for index in PACKAGES.len()..256 {
            std::fs::write(root.path().join(format!("core/{index:03}.yaml")), b"{}\n").unwrap();
        }
        let rules = packaged_rules_from(root.path()).unwrap();
        assert_eq!(rules.len(), 256);
        assert!(
            rules
                .windows(2)
                .all(|pair| { pair[0].package != pair[1].package || pair[0].path < pair[1].path })
        );
        std::fs::write(root.path().join("core/extra.yaml"), b"{}\n").unwrap();
        assert!(
            packaged_rules_from(root.path())
                .unwrap_err()
                .to_string()
                .contains("256 files")
        );
    }

    #[cfg(unix)]
    #[test]
    fn asset_symlinks_allow_regular_targets_and_reject_special_targets() {
        let root = tempfile::tempdir().unwrap();
        let regular = root.path().join("regular");
        let link = root.path().join("rule.yaml");
        std::fs::write(&regular, b"{}\n").unwrap();
        std::os::unix::fs::symlink(&regular, &link).unwrap();
        assert_eq!(read_asset(&link, "rule.yaml", 3).unwrap(), b"{}\n");
        assert!(read_asset(&link, "rule.yaml", 2).is_err());
        std::fs::remove_file(&link).unwrap();
        let socket = root.path().join("socket");
        let _listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
        std::os::unix::fs::symlink(socket, &link).unwrap();
        assert!(
            read_asset(&link, "rule.yaml", 3)
                .unwrap_err()
                .to_string()
                .contains("regular files")
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn actual_asset_bytes_count_toward_the_aggregate_budget() {
        let root = tempfile::tempdir().unwrap();
        for package in PACKAGES {
            std::fs::create_dir(root.path().join(package)).unwrap();
            std::fs::write(root.path().join(package).join("rule.yaml"), b"{}\n").unwrap();
        }
        std::fs::write(
            root.path().join("core/rule.yaml"),
            vec![b' '; super::super::MAX_CONFIG_BYTES - (PACKAGES.len() - 1) * 3],
        )
        .unwrap();
        let rules = packaged_rules_from(root.path()).unwrap();
        assert_eq!(
            rules.iter().map(|rule| rule.bytes.len()).sum::<usize>(),
            super::super::MAX_CONFIG_BYTES
        );
        // procfs reports a regular file with length zero; its actual bytes are
        // nonempty. This exercises accounting independently of a growth race.
        let source = Path::new("/proc/version");
        let metadata = std::fs::metadata(source).unwrap();
        assert!(metadata.is_file());
        assert_eq!(metadata.len(), 0);
        std::os::unix::fs::symlink(source, root.path().join("core/extra.yaml")).unwrap();
        match packaged_rules_from(root.path()) {
            Ok(rules) => panic!(
                "Catalog accepted {} actual bytes despite its 1 MiB budget",
                rules.iter().map(|rule| rule.bytes.len()).sum::<usize>()
            ),
            Err(error) => assert!(error.to_string().contains("1 MiB"), "{error:#}"),
        }
    }
}
