//! Repository-relative patterns and the checks that bind them to captured inputs.
use super::resource_exclusions::{Evidence, Policy};
use anyhow::{Result, bail, ensure};
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};

pub fn matcher(patterns: &[String]) -> Result<GlobSet> {
    if patterns.len() > 256 {
        bail!("exclude exceeds 256 patterns");
    }
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        if pattern.is_empty()
            || pattern.len() > 1024
            || pattern.starts_with(['/', '!'])
            || pattern.contains(['\\', ':'])
            || pattern.chars().any(char::is_control)
            || pattern.split('/').any(|part| part == ".." || part == ".")
        {
            bail!(
                "exclude requires repository-relative /-separated globs without negation: {pattern:?}"
            );
        }
        builder.add(
            GlobBuilder::new(pattern)
                .literal_separator(true)
                .case_insensitive(false)
                .build()?,
        );
    }
    Ok(builder.build()?)
}

pub fn validate_excluded(excluded_paths: &[String], protected_paths: &[String]) -> Result<()> {
    let mut builder = GlobSetBuilder::new();
    for pattern in protected_paths {
        builder.add(globset::Glob::new(pattern)?);
    }
    let matcher = builder.build()?;
    for path in excluded_paths {
        ensure!(
            !matcher.is_match(path),
            "exclude matches a protected verification input: {path}"
        );
    }
    Ok(())
}

pub fn validate_capture(
    exclude: &[String],
    policy: &Policy,
    protected_paths: &[String],
    captured_exclude: &[String],
    excluded_paths: &[String],
    captured: &Evidence,
) -> Result<()> {
    ensure!(
        exclude == captured_exclude
            && policy == &captured.policy
            && protected_paths == captured.protected_paths.as_slice(),
        "Exclusion policy changed during acquisition"
    );
    validate_excluded(excluded_paths, protected_paths)
}
