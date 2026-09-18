//! Runs the shipped corpus through production evaluators without project commands.

use crate::{
    adapters,
    config::{
        self, Config,
        catalog::Catalog,
        selfcheck::{Case, Input},
    },
    domain::{
        self, Decision, VerificationBoundary,
        selfcheck::{FixtureResult, SelfcheckReport, compare_golden},
    },
    snapshot::{self, File, Snapshot},
};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::{Duration, Instant},
};

pub async fn run(fixture: Option<String>, rule: Option<String>) -> SelfcheckReport {
    let (selected_fixture, selected_rule) = (fixture.clone(), rule.clone());
    match tokio::task::spawn_blocking(move || execute(selected_fixture, selected_rule)).await {
        Ok(report) => report,
        Err(error) => {
            let mut report = empty_report(fixture, rule);
            report
                .errors
                .push(format!("Selfcheck worker failed: {error}"));
            report
        }
    }
}

fn empty_report(fixture: Option<String>, rule: Option<String>) -> SelfcheckReport {
    SelfcheckReport {
        schema_version: 1,
        tool_version: env!("CARGO_PKG_VERSION").into(),
        corpus_digest: snapshot::digest(config::selfcheck::SUITES.iter().flat_map(|(suite, path, cases, golden_path, golden)| [*suite, *path, *cases, *golden_path, *golden]).collect::<String>().as_bytes()),
        rules_digest: String::new(),
        fixture_filter: fixture,
        rule_filter: rule,
        decision: Decision::Incomplete,
        complete: false,
        fixtures: vec![],
        errors: vec![],
        verification: VerificationBoundary {
            conclusion: VerificationBoundary::conclusion(Decision::Incomplete).into(),
            verified_shapes: vec![],
            known_limits: vec![
                "Golden agreement only covers the selected named fixtures and assertions; it is falsification evidence, not proof of production correctness.".into(),
                "Synthetic project/report facts do not execute Maven, Python, compilers or external analyzers.".into(),
                "Filtered selfcheck is partial regression evidence; run the full corpus before delivery.".into(),
                "Policy fixtures use public synthetic keys and disposable archives; they grant no real approval and do not establish production rollout or downstream benefit.".into(),
            ],
            unverified_assumptions: vec![
                "Unrepresented frameworks, configuration centers, runtime behavior and third-party tool versions remain unverified.".into(),
                "Custom repository rules require repository-owned fixtures; bundled custom-DSL fixtures cover only the declared evaluator shapes.".into(),
            ],
        },
    }
}

fn execute(fixture: Option<String>, rule: Option<String>) -> SelfcheckReport {
    let mut report = empty_report(fixture, rule);
    let result = (|| -> Result<()> {
        let started = Instant::now();
        let config = Config {
            rulesets: vec![
                "core".into(),
                "shared".into(),
                "lang-java".into(),
                "lang-python".into(),
                "lang-typescript".into(),
            ],
            ..Config::default()
        };
        let catalog = Catalog::load(&config, std::iter::empty())
            .context("Cannot load active rule assets for selfcheck")?;
        report.rules_digest = snapshot::digest(&serde_json::to_vec(&catalog)?);
        let cases = config::selfcheck::load()?;
        validate_coverage(&cases, &catalog)?;
        for case in cases.into_iter().filter(|case| {
            report
                .fixture_filter
                .as_ref()
                .is_none_or(|filter| filter == &case.suite)
                && report
                    .rule_filter
                    .as_ref()
                    .is_none_or(|filter| filter == &case.fixture.rule)
        }) {
            if started.elapsed() > Duration::from_secs(60) {
                bail!(
                    "Selfcheck exceeded its 60 second corpus budget before {}/{}",
                    case.suite,
                    case.fixture.id
                );
            }
            let input_bytes = serde_json::to_vec(&case.fixture.input)?;
            let mut result = FixtureResult {
                fixture: case.fixture.id.clone(),
                suite: case.suite.clone(),
                rule: case.fixture.rule.clone(),
                input: case.input.clone(),
                input_digest: snapshot::digest(&input_bytes),
                golden_digest: snapshot::digest(&serde_json::to_vec(&case.golden)?),
                decision: Decision::Incomplete,
                mismatches: vec![],
                error: None,
                observed: None,
            };
            match observe(&case, &catalog) {
                Ok(observed) => {
                    result.mismatches = compare_golden(&case.golden, &observed);
                    result.decision = if result.mismatches.is_empty() {
                        Decision::Pass
                    } else {
                        Decision::Fail
                    };
                    result.observed = Some(observed);
                }
                Err(error) => {
                    result.error = Some(format!(
                        "Fixture {}/{}; input {}; assertion execution: {error:#}",
                        case.suite, case.fixture.id, case.input
                    ))
                }
            }
            if result.decision == Decision::Pass {
                report.verification.verified_shapes.push(format!(
                    "{}/{}: {}",
                    case.suite, case.fixture.id, case.fixture.description
                ));
            }
            report.fixtures.push(result);
        }
        if report.fixtures.is_empty() {
            bail!("No fixtures match the requested suite/rule; empty selfcheck is incomplete");
        }
        Ok(())
    })();
    if let Err(error) = result {
        report.errors.push(format!("{error:#}"));
    }
    report.decision = if !report.errors.is_empty()
        || report
            .fixtures
            .iter()
            .any(|case| case.decision == Decision::Incomplete)
    {
        Decision::Incomplete
    } else if report
        .fixtures
        .iter()
        .any(|case| case.decision == Decision::Fail)
    {
        Decision::Fail
    } else {
        Decision::Pass
    };
    report.complete = report.decision != Decision::Incomplete;
    report.verification.conclusion = VerificationBoundary::conclusion(report.decision).into();
    report
}

fn validate_coverage(cases: &[Case], catalog: &Catalog) -> Result<()> {
    for rule in catalog.entries.keys() {
        for suite in ["minimal", "typical", "stress"] {
            let verdicts: BTreeSet<_> = cases
                .iter()
                .filter(|case| case.suite == suite && &case.fixture.rule == rule)
                .filter_map(|case| case.golden.get("/verdict").and_then(Value::as_str))
                .collect();
            if !["pass", "fail"]
                .iter()
                .all(|verdict| verdicts.contains(verdict))
            {
                bail!("Rule {rule} lacks independent compliant/violating goldens in {suite}");
            }
        }
    }
    Ok(())
}

fn observe(case: &Case, catalog: &Catalog) -> Result<Value> {
    match &case.fixture.input {
        Input::Evolution { scenario } => super::selfcheck_policy::observe(scenario),
        Input::Manual {
            record,
            expected,
            now,
            tamper,
        } => super::selfcheck_evidence::manual(record, expected, *now, *tamper),
        Input::Compatibility { content, classes } => Ok(
            match adapters::compatibility::parse(
                content.as_bytes(),
                "old.jar",
                "new.jar",
                classes,
                config::compatibility::CompatibilityLevel::Both,
                true,
            ) {
                Ok(data) => json!({"status":"completed", "data":data}),
                Err(error) => json!({"status":"incomplete", "reason":format!("{error:#}")}),
            },
        ),
        Input::Snapshot { scenario } => tokio::runtime::Handle::current().block_on(async {
            tokio::time::timeout(
                Duration::from_secs(10),
                super::selfcheck_io::snapshot(*scenario),
            )
            .await
            .context("Native snapshot fixture exceeded 10 seconds")?
        }),
        Input::Runner { scenario } => {
            tokio::runtime::Handle::current().block_on(super::selfcheck_io::runner(*scenario))
        }
        Input::Rule {
            parameters,
            files,
            binary_files,
            base_files,
            commits,
            projects,
            custom,
        } => {
            let convert = |files: &BTreeMap<String, String>| {
                files
                    .iter()
                    .map(|(path, text)| {
                        (
                            path.clone(),
                            File {
                                bytes: text.as_bytes().to_vec(),
                                executable: false,
                            },
                        )
                    })
                    .collect::<BTreeMap<_, _>>()
            };
            let (mut files, base_files) = (convert(files), convert(base_files));
            for (path, bytes) in binary_files {
                if files
                    .insert(
                        path.clone(),
                        File {
                            bytes: bytes.clone(),
                            executable: false,
                        },
                    )
                    .is_some()
                {
                    bail!("Fixture contains duplicate text/binary input: {path}");
                }
            }
            let digest = snapshot::content_digest(&files);
            let snapshot = Snapshot {
                root: ".".into(),
                identity: snapshot::Identity {
                    mode: "fixture".into(),
                    base: "fixture-base".into(),
                    head: "fixture-head".into(),
                    content_digest: digest.clone(),
                    merge_request: None,
                },
                changes: snapshot::compare_files(&base_files, &files),
                files,
                base_files,
                path_filter: None,
                commits: commits
                    .iter()
                    .enumerate()
                    .map(|(i, message)| (format!("fixture-commit-{i}"), message.clone()))
                    .collect(),
            };
            let mut projects = projects.clone();
            for project in &mut projects {
                if project.snapshot_digest == "$snapshot" {
                    project.snapshot_digest.clone_from(&digest);
                }
            }
            let check = if let Some(custom) = custom {
                adapters::custom_rules::evaluate_with_projects(
                    custom,
                    &config::RuleSetting::default(),
                    &snapshot,
                    &projects,
                )
            } else {
                let builtin = catalog
                    .entries
                    .get(&case.fixture.rule)
                    .and_then(|entry| entry.builtin.as_ref())
                    .context("Fixture rule is absent from active catalog")?;
                let mut setting = builtin.defaults.clone();
                setting.parameters.extend(parameters.clone());
                adapters::rules::evaluate_with_projects(
                    &case.fixture.rule,
                    &builtin.implementation,
                    &setting,
                    &snapshot,
                    &projects,
                )
            };
            let mut observed = serde_json::to_value(&check)?;
            observed["diagnostic_count"] = json!(check.diagnostics.len());
            Ok(observed)
        }
        Input::Report { format, content } => Ok(
            match adapters::reports::parse(*format, content.as_bytes()) {
                Ok(data) => {
                    json!({"status":"completed", "findings":data.has_findings(), "data":data})
                }
                Err(error) => json!({"status":"incomplete", "reason":format!("{error:#}")}),
            },
        ),
        Input::Policy { content } => Ok(match config::parse(content.as_bytes()) {
            Ok(_) => json!({"status":"completed"}),
            Err(error) => json!({"status":"incomplete", "reason":format!("{error:#}")}),
        }),
        Input::Gate {
            checks,
            required,
            invalid,
        } => Ok(serde_json::to_value(domain::evaluate(
            checks, required, invalid,
        ))?),
    }
}

/// Fixed child-process behaviors used to exercise the installed runner itself.
pub async fn probe(mode: &str) -> (String, u8) {
    match mode {
        "timeout" => {
            tokio::time::sleep(Duration::from_secs(3)).await;
            ("late".into(), 0)
        }
        "failure" => ("fixture failure".into(), 1),
        "overflow" => ("x".repeat(crate::runner::MAX_OUTPUT_BYTES + 1), 0),
        _ => ("fixture success".into(), 0),
    }
}
