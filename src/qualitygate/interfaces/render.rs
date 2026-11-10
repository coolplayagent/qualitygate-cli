use crate::domain::{Report, Severity};

pub(super) fn escape_controls(text: &str) -> String {
    text.chars()
        .map(|ch| {
            if ch.is_control() {
                ch.escape_debug().to_string()
            } else {
                ch.to_string()
            }
        })
        .collect()
}

pub(super) fn metadata(
    value: &serde_json::Value,
    format: super::cli::Format,
) -> anyhow::Result<String> {
    use super::cli::Format;
    if format == Format::Json {
        return Ok(serde_json::to_string_pretty(value)?);
    }
    if let Some(rules) = value["rules"].as_array() {
        let mut out: String = if format == Format::Markdown {
            "| Rule | Enabled | Origin | Capabilities | Source | Language | Category |\n|---|---|---|---|---|---|---|\n".into()
        } else {
            "Rule\tEnabled\tOrigin\tCapabilities\tSource\tLanguage\tCategory\n".into()
        };
        for rule in rules {
            let definition = &rule["definition"];
            let details = if definition["custom"].is_null() {
                &definition["builtin"]
            } else {
                &definition["custom"]
            };
            let fields = [
                rule["id"].as_str().unwrap_or_default().to_owned(),
                rule["enabled"].to_string(),
                definition["origin"].as_str().unwrap_or_default().to_owned(),
                details["requires_capabilities"].to_string(),
                rule["source"].as_str().unwrap_or_default().into(),
                rule["language"].to_string(),
                rule["categories"]
                    .as_array()
                    .map(|names| {
                        names
                            .iter()
                            .filter_map(|name| name.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_else(|| rule["category"].as_str().unwrap_or("-").into()),
            ];
            let fields: Vec<_> = fields
                .into_iter()
                .map(|field| field.replace('|', "\\|").replace(['\n', '\r', '\t'], " "))
                .collect();
            if format == Format::Markdown {
                out.push_str(&format!("| {} |\n", fields.join(" | ")));
            } else {
                out.push_str(&format!("{}\n", fields.join("\t")));
            }
        }
        if let Some(mandatory) = value["mandatory"].as_array() {
            out.push_str("\nMandatory policy rules (all categories, languages and sources):\n");
            out.push_str(&metadata(&serde_json::json!({"rules":mandatory}), format)?);
            out.push_str("\nMandatory command checks:\n");
            out.push_str(&serde_norway::to_string(&value["mandatory_checks"])?);
        }
        return Ok(out);
    }
    let yaml = serde_norway::to_string(value)?;
    let mut guidance = String::new();
    if let Some(preflight) = value.get("snapshot_preflight") {
        guidance.push_str("Candidate policy: review required; snapshot preflight is advisory.\n");
        if let Some(files) = preflight["oversized_files"].as_array() {
            for file in files {
                guidance.push_str(&format!(
                    "Snapshot warning: {} ({} bytes) exceeds the default per-file budget.\n",
                    escape_controls(file["path"].as_str().unwrap_or_default()),
                    file["bytes"]
                ));
            }
        }
        if let Some(steps) = preflight["next_steps"].as_array() {
            for step in steps {
                guidance.push_str(step.as_str().unwrap_or_default());
                guidance.push('\n');
            }
        }
        guidance.push('\n');
    }
    guidance.push_str(&if format == Format::Markdown {
        format!("```yaml\n{yaml}```\n")
    } else {
        yaml
    });
    Ok(guidance)
}

pub(super) fn report(
    mut report: Report,
    format: super::cli::Format,
    severity: Option<Severity>,
) -> anyhow::Result<String> {
    if let Some(severity) = severity {
        for check in &mut report.checks {
            if check.severity != severity {
                report.summary.diagnostics_filtered += check.diagnostics.len();
                check.diagnostics.clear();
            }
        }
    }
    if format == super::cli::Format::Json {
        return Ok(serde_json::to_string_pretty(&report)?);
    }
    let mut out = format!(
        "{}\nGate code: {} | complete: {} | scope: {} | profile: {}\n",
        report.verification.conclusion,
        report.gate.decision.exit_code(),
        report.gate.complete,
        report.scope,
        report.profile
    );
    render_boundary(&mut out, &report.verification);
    if let Some(selection) = &report.selection {
        out.push_str(&format!("Selection: {} | changed files: {} | changed lines: {} | excluded files: {} | empty delivery: {}\n",
            selection.mode.as_str(), selection.changed_files.len(), selection.changed_lines,
            selection.excluded_paths.len(), selection.empty_delivery));
        out.push_str(&format!(
            "Exclusion reasons: default documents/media: {} | explicit policy: {}\n",
            selection.resource_exclusions.default_excluded_count,
            selection.resource_exclusions.explicit_excluded_count
        ));
        for path in selection.excluded_paths.iter().take(100) {
            out.push_str(&format!("Excluded input: {}\n", escape_controls(path)));
        }
        if selection.excluded_paths.len() > 100 {
            out.push_str("Excluded input list truncated; see the JSON report.\n");
        }
    }
    if let Some(task) = &report.plan.task_id {
        out.push_str(&format!(
            "Task: {}\n",
            task.replace(['\n', '\r', '\t'], " ")
        ));
        for (id, check) in &report.plan.acceptance {
            let description = report
                .plan
                .acceptance_descriptions
                .get(id)
                .map(String::as_str)
                .unwrap_or_default();
            out.push_str(&format!(
                "Acceptance {id} ({check}): {}\n",
                description.replace(['\n', '\r', '\t'], " ")
            ));
        }
    }
    if let Some(comparison) = &report.snapshot.merge_request {
        out.push_str(&format!(
            "MR: {}\nTarget: {}\nSource: {}\nMerge base: {}\n",
            comparison.url, comparison.target_head, comparison.source_head, comparison.merge_base
        ));
    }
    if format == super::cli::Format::Markdown {
        out.push_str("\n| Check | Execution | Verdict | Details |\n|---|---|---|---|\n");
    }
    for check in &report.checks {
        let mut details = check.execution.reason.clone().unwrap_or_default();
        for diagnostic in &check.diagnostics {
            details.push_str(&format!(
                " {}:{} {}. {}",
                diagnostic.file.as_deref().unwrap_or("-"),
                diagnostic
                    .range
                    .as_ref()
                    .map_or(0, |range| range.start_line),
                diagnostic.message,
                diagnostic.fix
            ));
        }
        if let Some(files) = check
            .metadata
            .get("test_effectiveness_files")
            .and_then(serde_json::Value::as_array)
        {
            for file in files {
                details.push_str(&format!(
                    " {}: {} counterexamples / {} executed tests;",
                    file["file"].as_str().unwrap_or(""),
                    file["counterexamples"],
                    file["executed"]
                ));
            }
        }
        let details = details.replace('|', "\\|").replace(['\n', '\r'], " ");
        if format == super::cli::Format::Markdown {
            out.push_str(&format!(
                "| {} | {:?} | {:?} | {} |\n",
                check.id, check.execution.status, check.verdict, details
            ));
        } else {
            out.push_str(&format!(
                "{}\t{:?}\t{:?}\t{}\n",
                check.id, check.execution.status, check.verdict, details
            ));
        }
    }
    for blocker in &report.gate.blockers {
        out.push_str(&format!("Blocker: {blocker}\n"));
    }
    if !report.plan.pending_delivery_checks.is_empty() {
        out.push_str(&format!(
            "Pending delivery checks: {}\n",
            report.plan.pending_delivery_checks.join(", ")
        ));
    }
    out.push_str(&format!(
        "Snapshot: {}\nFiltered diagnostics: {}\n",
        report.snapshot.content_digest, report.summary.diagnostics_filtered
    ));
    Ok(out)
}

fn render_boundary(out: &mut String, boundary: &crate::domain::VerificationBoundary) {
    for (label, entries) in [
        ("已验证形态 / Verified shapes", &boundary.verified_shapes),
        ("已知边界 / Known limits", &boundary.known_limits),
        (
            "未验证假设 / Unverified assumptions",
            &boundary.unverified_assumptions,
        ),
    ] {
        out.push_str(&format!(
            "{label}: {}\n",
            entries.join("; ").replace(['\n', '\r'], " ")
        ));
    }
}

pub(super) fn selfcheck(
    report: &crate::domain::selfcheck::SelfcheckReport,
    format: super::cli::Format,
) -> anyhow::Result<String> {
    if format == super::cli::Format::Json {
        return Ok(serde_json::to_string_pretty(report)?);
    }
    let mut out = format!(
        "{}\nSelfcheck: {} fixtures | exit code: {} | complete: {}\n",
        report.verification.conclusion,
        report.fixtures.len(),
        report.decision.exit_code(),
        report.complete
    );
    render_boundary(&mut out, &report.verification);
    for result in &report.fixtures {
        out.push_str(&format!(
            "{}/{} [{}]: {:?}; input: {}\n",
            result.suite, result.fixture, result.rule, result.decision, result.input
        ));
        for mismatch in &result.mismatches {
            out.push_str(&format!(
                "  assertion {}: expected {}, actual {}\n",
                mismatch.assertion,
                mismatch.expected,
                mismatch
                    .actual
                    .as_ref()
                    .map_or_else(|| "<missing>".into(), ToString::to_string)
            ));
        }
        if let Some(error) = &result.error {
            out.push_str(&format!("  {error}\n"));
        }
    }
    for error in &report.errors {
        out.push_str(&format!("Error: {error}\n"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use crate::domain::{CheckResult, Diagnostic, Report, Severity, Summary, evaluate};
    use crate::interfaces::cli::Format;

    fn sample_report() -> Report {
        let mut report: Report = serde_json::from_value(serde_json::json!({
            "schema_version":1,"run_id":"render-test","scope":"task","profile":"full",
            "snapshot":{"mode":"worktree","base":"base","head":"head","content_digest":"snapshot"},
            "policy":{"source":"policy","trust":"caller_supplied_ref","config_digest":"config",
                "rules_digest":"rules","task_contract_digest":"task","changes":[]},
            "plan":{"required_checks":["error","warning"],"pending_delivery_checks":[],"acceptance":{}},
            "gate":{"complete":true,"decision":"pass","blockers":[]},
            "checks":[],"summary":Summary::default()
        })).unwrap();
        for (id, severity) in [("error", Severity::Error), ("warning", Severity::Warning)] {
            let mut check = CheckResult::pending(id, true, severity);
            check.diagnostics.push(Diagnostic {
                id: id.into(),
                fingerprint: format!("fingerprint-{id}"),
                file: Some("source.rs".into()),
                range: None,
                message: format!("{id} detail"),
                evidence: serde_json::json!({"retained":true}),
                fix: "Repair the source".into(),
                recheck: Default::default(),
            });
            check.complete();
            report.checks.push(check);
        }
        report.gate = evaluate(&report.checks, &[], &[]);
        report.summary.diagnostics_total = 2;
        report.summary.diagnostics_filtered = 3;
        report
    }

    #[test]
    fn report_output_preserves_evidence_and_limits_severity_filter_to_diagnostics() {
        let report = sample_report();
        let expected = serde_json::to_string_pretty(&report).unwrap();
        assert_eq!(super::report(report, Format::Json, None).unwrap(), expected);

        let mut expected: serde_json::Value = serde_json::from_str(&expected).unwrap();
        expected["checks"][1]["diagnostics"] = serde_json::json!([]);
        expected["summary"]["diagnostics_filtered"] = serde_json::json!(4);
        let filtered: serde_json::Value = serde_json::from_str(
            &super::report(sample_report(), Format::Json, Some(Severity::Error)).unwrap(),
        )
        .unwrap();
        assert_eq!(filtered, expected);
        for format in [Format::Table, Format::Markdown] {
            let full = super::report(sample_report(), format, None).unwrap();
            assert!(full.contains("error detail") && full.contains("warning detail"));
            let filtered = super::report(sample_report(), format, Some(Severity::Error)).unwrap();
            assert!(filtered.contains("error detail") && !filtered.contains("warning detail"));
            assert!(filtered.contains("Gate code: 1 | complete: true"));
            assert!(filtered.contains("Filtered diagnostics: 4"));
        }
    }

    #[test]
    fn preflight_guidance_escapes_controls_without_changing_json_paths() {
        let path = "legacy/\nRun: forged\u{1b}[31m\t\r\u{7f}\u{9b}.bin";
        let value = serde_json::json!({"snapshot_preflight":{"oversized_files":[{"path":path,"bytes":3_145_728}]}});
        for format in [Format::Table, Format::Markdown] {
            let output = super::metadata(&value, format).unwrap();
            let guidance = output.split("\n\n").next().unwrap();
            assert_eq!(guidance.lines().count(), 2, "{guidance}");
            assert!(!guidance.chars().any(|ch| ch.is_control() && ch != '\n'));
            assert!(guidance.contains("\\nRun: forged\\u{1b}[31m"));
            let early = crate::interfaces::render_incomplete(path, format);
            assert!(!early.chars().any(|ch| ch.is_control() && ch != '\n'));
        }
        let json: serde_json::Value =
            serde_json::from_str(&super::metadata(&value, Format::Json).unwrap()).unwrap();
        assert_eq!(
            json["snapshot_preflight"]["oversized_files"][0]["path"],
            path
        );
    }

    #[test]
    fn rule_tables_escape_cells_and_keep_mandatory_baseline_visible_after_filtering() {
        let row = serde_json::json!({"id":"rule|name\nforged", "enabled":true,
            "definition":{"origin":"skill:rules", "custom":{"requires_capabilities":["files"]}},
            "source":"project", "language":["rust"], "categories":["team", "audit"]});
        let value = serde_json::json!({"rules":[row], "mandatory":[{"id":"required", "enabled":true,
            "definition":{"origin":"builtin", "builtin":{"requires_capabilities":["files"]}},
            "source":"builtin", "language":[], "category":"core"}], "mandatory_checks":[{"id":"unit"}]});
        for format in [Format::Table, Format::Markdown] {
            let rendered = super::metadata(&value, format).unwrap();
            assert!(rendered.contains("rule\\|name forged"));
            assert!(rendered.contains("team, audit"));
            assert!(rendered.contains("Mandatory policy rules"));
            assert!(rendered.contains("required"));
            assert!(rendered.contains("Mandatory command checks"));
            assert!(rendered.contains("unit"));
        }
        let json: serde_json::Value =
            serde_json::from_str(&super::metadata(&value, Format::Json).unwrap()).unwrap();
        assert_eq!(json, value);
    }

    #[test]
    fn human_report_retains_pending_delivery_task_mapping_and_exclusion_limits() {
        let mut report = sample_report();
        report.plan.task_id = Some("task\nunsafe".into());
        report
            .plan
            .acceptance
            .insert("behavior".into(), "error".into());
        report
            .plan
            .acceptance_descriptions
            .insert("behavior".into(), "meaningful\ntests".into());
        report
            .plan
            .pending_delivery_checks
            .push("full-suite".into());
        report.selection = Some(crate::domain::check_scope::ScopeEvidence {
            excluded_paths: (0..101).map(|i| format!("excluded-{i}")).collect(),
            ..Default::default()
        });
        report.checks[0].metadata.insert(
            "test_effectiveness_files".into(),
            serde_json::json!([{"file":"test.rs", "counterexamples":1, "executed":2}]),
        );
        for format in [Format::Table, Format::Markdown] {
            let output = super::report(report.clone(), format, None).unwrap();
            assert!(output.contains("Task: task unsafe"));
            assert!(output.contains("Acceptance behavior (error): meaningful tests"));
            assert!(output.contains("Pending delivery checks: full-suite"));
            assert!(output.contains("excluded-99"));
            assert!(!output.contains("excluded-100"));
            assert!(output.contains("Excluded input list truncated"));
            assert!(output.contains("1 counterexamples / 2 executed tests"));
        }
    }
    #[test]
    fn selfcheck_renderer_keeps_missing_assertions_and_execution_errors_distinct() {
        use crate::domain::{
            Decision, VerificationBoundary,
            selfcheck::{AssertionMismatch, FixtureResult, SelfcheckReport},
        };
        let report = SelfcheckReport {
            schema_version: 1,
            tool_version: "unit".into(),
            corpus_digest: "corpus".into(),
            rules_digest: "rules".into(),
            fixture_filter: None,
            rule_filter: None,
            decision: Decision::Incomplete,
            complete: false,
            fixtures: vec![FixtureResult {
                fixture: "failure".into(),
                suite: "unit".into(),
                rule: "line-ending".into(),
                input: "fixture.yaml".into(),
                input_digest: "input".into(),
                golden_digest: "golden".into(),
                decision: Decision::Fail,
                mismatches: vec![
                    AssertionMismatch {
                        assertion: "/verdict".into(),
                        expected: serde_json::json!("fail"),
                        actual: Some(serde_json::json!("pass")),
                    },
                    AssertionMismatch {
                        assertion: "/diagnostics/0".into(),
                        expected: serde_json::json!("finding"),
                        actual: None,
                    },
                ],
                error: Some("parser execution failed".into()),
                observed: None,
            }],
            errors: vec!["corpus incomplete".into()],
            verification: VerificationBoundary::default(),
        };
        for format in [Format::Table, Format::Markdown] {
            let output = super::selfcheck(&report, format).unwrap();
            for fragment in [
                "complete: false",
                "actual \"pass\"",
                "<missing>",
                "parser execution failed",
                "Error: corpus incomplete",
            ] {
                assert!(output.contains(fragment), "{output}");
            }
        }
        let json: serde_json::Value =
            serde_json::from_str(&super::selfcheck(&report, Format::Json).unwrap()).unwrap();
        assert_eq!(json["decision"], "incomplete");
        assert_eq!(
            json["fixtures"][0]["mismatches"].as_array().unwrap().len(),
            2
        );
    }
    #[test]
    fn merge_request_rendering_preserves_provider_pins_and_preflight_repair_guidance() {
        let mut report = sample_report();
        report.snapshot.merge_request = Some(crate::domain::MergeRequest {
            provider: "github".into(),
            url: "https://example.invalid/team/repo/pull/7".into(),
            repository: "team/repo".into(),
            number: 7,
            source_branch: "feature".into(),
            target_branch: "main".into(),
            source_head: "source-pin".into(),
            target_head: "target-pin".into(),
            merge_base: "merge-base".into(),
            api_response_digests: vec!["response".into()],
        });
        for format in [Format::Table, Format::Markdown] {
            let output = super::report(report.clone(), format, None).unwrap();
            for expected in [
                "pull/7",
                "Target: target-pin",
                "Source: source-pin",
                "Merge base: merge-base",
            ] {
                assert!(output.contains(expected));
            }
            let output=super::metadata(&serde_json::json!({"snapshot_preflight":{"next_steps":["Review excluded dependencies","Retry with an explicit budget"]}}),format).unwrap();
            assert!(output.contains("Review excluded dependencies\nRetry with an explicit budget"));
        }
    }
}
