//! Strict repository-relative exclusion patterns.
use anyhow::{Result, bail};
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

pub fn protected_paths(
    config: &super::Config,
    configuration: &str,
    task: Option<&str>,
) -> Vec<String> {
    let mut paths = config.verification_assets.clone();
    paths.push(globset::escape(configuration));
    paths.extend(task.map(globset::escape));
    if let Some(directory) = super::catalog::project_rules_directory(config) {
        paths.push(format!("{}/**", globset::escape(directory)));
    }
    paths.extend(
        config
            .rules
            .values()
            .filter_map(|rule| rule.source.as_ref())
            .map(|source| globset::escape(&source.document)),
    );
    paths
}
