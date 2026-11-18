//! Resolve policy configuration and rule sources to protected snapshot inputs.

use crate::config::{Config, catalog};
pub use crate::snapshot_scope::exclusions::matcher;

pub fn protected_paths(config: &Config, configuration: &str, task: Option<&str>) -> Vec<String> {
    let mut paths = config.verification_assets.clone();
    paths.extend(
        config
            .checks
            .iter()
            .flat_map(|check| &check.tools)
            .flat_map(|tool| &tool.inputs)
            .map(|path| globset::escape(path)),
    );
    paths.extend(
        config
            .rules
            .values()
            .filter_map(|rule| rule.provenance.as_ref())
            .map(|provenance| globset::escape(&provenance.evidence_file)),
    );
    paths.push(globset::escape(configuration));
    paths.extend(task.map(globset::escape));
    paths.extend(
        config
            .agent_obligations
            .iter()
            .map(|o| globset::escape(&o.source.document)),
    );
    paths.extend(
        config
            .constraints
            .iter()
            .map(|c| globset::escape(&c.source.document)),
    );
    if let Some(directory) = catalog::project_rules_directory(config) {
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

pub fn resolved(
    config: &Config,
    catalog: &catalog::Catalog,
    configuration: &str,
    task: Option<&str>,
) -> Vec<String> {
    let mut paths = protected_paths(config, configuration, task);
    paths.extend(
        catalog
            .entries
            .values()
            .filter_map(|entry| entry.custom.as_ref())
            .map(|rule| globset::escape(&rule.source.document)),
    );
    paths.sort();
    paths.dedup();
    paths
}
