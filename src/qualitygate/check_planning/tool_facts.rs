//! Validate fact output contracts before any producer is executed.

use crate::{
    config::{CheckKind, CommandCheck, Config, IncrementMode},
    tool_evidence::contracts::ToolFormat,
};
use anyhow::{Result, ensure};

pub(crate) fn validate(check: &CommandCheck, config: &Config) -> Result<()> {
    let Some(output) = &check.tool_facts else {
        return Ok(());
    };
    output.validate()?;
    if output.adapter != ToolFormat::ToolFacts {
        ensure!(
            output.native_runtime.is_some(),
            "Native fact commands require an explicitly provisioned parser runtime"
        );
        ensure!(
            check
                .tools
                .iter()
                .any(|tool| tool.id == "tree-sitter-language-pack"),
            "Native fact commands require a language-pack version probe"
        );
    }
    ensure!(
        check.kind == CheckKind::Command
            && check.compatibility.is_none()
            && check.test_effectiveness.is_none(),
        "Tool facts require an ordinary command producer"
    );
    ensure!(
        check.reports.is_empty() && check.projects.is_empty(),
        "A tool fact output cannot share a command with diagnostic or project reports"
    );
    ensure!(
        check.expected_exit_code == 0,
        "Tool fact collection success requires exit code zero"
    );
    ensure!(
        check.tools.iter().any(|tool| tool.id == output.tool_id),
        "Tool fact producer needs its declared executable version probe"
    );
    ensure!(
        check.findings_exit_codes.iter().all(|code| *code > 0),
        "Tool fact findings exits must be positive codes"
    );
    match output.adapter {
        ToolFormat::Crapper => ensure!(
            check.findings_exit_codes.iter().all(|code| *code == 2),
            "Pinned crapper only declares findings exit 2"
        ),
        ToolFormat::Dryer => ensure!(
            check.findings_exit_codes.is_empty(),
            "Pinned dryer does not declare findings exit codes"
        ),
        ToolFormat::ToolFacts => {}
    }
    if let Some(id) = &output.coverage_producer {
        ensure!(
            id != &check.id && check.depends_on.contains(id),
            "Coverage producer must be an explicit prerequisite"
        );
        let producer = config
            .checks
            .iter()
            .find(|candidate| &candidate.id == id)
            .ok_or_else(|| anyhow::anyhow!("Coverage producer is missing: {id}"))?;
        ensure!(
            producer.kind == CheckKind::Command
                && producer.tool_facts.is_none()
                && !producer.reports.is_empty()
                && producer
                    .reports
                    .iter()
                    .any(|report| !report.coverage_paths.is_empty()
                        && report.mode == IncrementMode::Full),
            "Coverage prerequisite must independently produce a full coverage report"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn config() -> Config {
        serde_json::from_value(json!({"schema_version":1,"checks":[{
            "id":"metrics","argv":["analyzer"],"tools":[{"id":"analyzer","argv":["analyzer","--version"]}],
            "tool_facts":{"adapter":"tool_facts","schema":"function_metric","path":"facts.json","tool_id":"analyzer","tool_version":"1","scope":{"project":"sample"},"paths":["src/lib.rs"],"crate_name":"sample","metrics":[{"metric":"complexity","unit":"count","measurement":"fixture-v1"}]}
        }]})).unwrap()
    }

    #[test]
    fn fact_contracts_require_an_independent_pinned_producer_and_explicit_coverage_dependency() {
        let mut config = config();
        validate(&config.checks[0], &config).unwrap();
        config.checks[0].tools.clear();
        assert!(validate(&config.checks[0], &config).is_err());
        config = self::config();
        config.checks[0]
            .tool_facts
            .as_mut()
            .unwrap()
            .coverage_producer = Some("coverage".into());
        assert!(validate(&config.checks[0], &config).is_err());
        config.checks[0].depends_on.push("coverage".into());
        assert!(validate(&config.checks[0], &config).is_err());
        config.checks.push(serde_json::from_value(json!({"id":"coverage","argv":["coverage"],"reports":[{"path":"coverage.lcov","format":"lcov","coverage_paths":["src/**"],"minimum_coverage":0}]})).unwrap());
        validate(&config.checks[0], &config).unwrap();
        config.checks[1].reports[0].mode = IncrementMode::ChangedLines;
        assert!(validate(&config.checks[0], &config).is_err());
    }
}
