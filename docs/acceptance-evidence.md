# Requirements acceptance evidence

This 2026-09-13 audit separates reproducible implementation evidence from the
team-owned real-repository evidence required for final acceptance. A passing
fixture, self-hosted check, or CI run proves the stated implementation behavior;
it does not establish pilot benefit, reviewer agreement, or production-policy
adoption.

## Functional acceptance (§9.1)

| Item | Reproducible implementation evidence | Acceptance status |
|---|---|---|
| 1. Applicability and errors | `tests/init.rs::generated_rust_candidate_runs_real_tests_then_repairs_and_rejects_zero_tests`, `tests/cli.rs::init_check_repair_and_staged_bytes_form_a_real_cli_loop`, `tests/cli.rs::missing_configuration_and_invalid_selection_return_incomplete`, `config/discovery/tests.rs::malformed_unsupported_and_dynamic_projects_do_not_gain_semantic_capabilities`, and `domain/gate_tests.rs::empty_or_only_skipped_never_claims_validation` | Verified in controlled repositories. Unsupported or incomplete inputs are reported, not passed. |
| 2. Gate completeness | `tests/cli.rs::command_preconditions_timeout_and_prerequisites_remain_incomplete_with_logs`, `required_tool_absence_blocks_even_when_violation_severity_is_warning`, and `zero_test_success_exit_and_stale_reports_cannot_pass`; `tests/execution.rs` covers missing reports, parser/probe failure, timeouts, preconditions, findings exits, and retained evidence; `domain/gate_tests.rs` covers 0/1/2 gate semantics | Verified in controlled repositories. Display filtering cannot alter the computed gate. |
| 3. Repair loop | `tests/init.rs::generated_rust_candidate_runs_real_tests_then_repairs_and_rejects_zero_tests`, `application/report_gate_tests.rs::test_statistics_cannot_claim_success_with_failures_or_missing_counts`, and `tests/cli.rs::zero_test_success_exit_and_stale_reports_cannot_pass` prove the required-test, assertion-failure, and zero-test protections. `tests/cli.rs::init_check_repair_and_staged_bytes_form_a_real_cli_loop` proves a controlled check/fix/recheck sequence. | **Partially verified.** The required real-pilot agent diagnosis, repair, and recheck remains unmeasured. |
| 4. Task acceptance | `tests/cli.rs::task_acceptance_is_merged_and_quick_reports_pending_delivery`, `tests/policy.rs::selected_policy_preserves_the_original_task_plan_after_candidate_deletion_or_corruption`, `tests/policy.rs::recheck_pins_the_selected_policy_commit_after_a_branch_moves`, and `config/plan.rs::reusable_planning_validates_task_contracts_without_requiring_yaml_loading` | Verified in controlled repositories. |
| 5. Increment accuracy | `application/report_gate_tests.rs::changed_lines_filters_old_issues_and_rejects_unlocated_diagnostics`, `new_diagnostics_match_multiplicity_and_survive_line_and_file_moves`, and `affected_scope_includes_unchanged_files_and_requires_explicit_impact_evidence`; `tests/cli.rs::baseline_analysis_filters_old_diagnostics_even_when_their_line_moves`; `snapshot/snapshot_tests.rs::rename_is_not_new_content_and_digest_includes_path_and_mode` | Verified for the documented report adapters and increment modes. Project-specific impact semantics remain declared adapter capabilities, not inferred. |
| 6. Snapshot consistency | `snapshot/snapshot_tests.rs::staged_snapshot_uses_index_bytes_and_worktree_includes_untracked_files`; `tests/execution.rs::mutating_commands_invalidate_evidence_and_cannot_be_repaired_by_later_commands`, `restored_bytes_mode_changes_and_symlinks_do_not_hide_input_mutations`, and `baseline_input_mutation_and_unexpected_exit_codes_cannot_pass_with_valid_reports`; `tests/policy.rs::staged_tasks_and_executable_mode_changes_use_their_selected_inputs` | Verified in controlled repositories. |
| 7. Rule and policy maintenance | `tests/policy.rs::selected_policy_preserves_the_original_task_plan_after_candidate_deletion_or_corruption`; `tests/source_reviews.rs::source_hash_refresh_alone_cannot_substitute_for_a_bound_review`; `tests/source_reviews.rs::review_records_follow_selected_policy_and_staged_source_bytes`; `tests/custom_rules.rs::changed_source_or_definition_invalidates_policy` | Technical detection and blocking are verified. Protected-reference selection and human approval are team-owned deployment evidence. |
| 8. Source-declaration boundary | `adapters/structure_rules_tests.rs::ai_scope_and_trailer_binding_without_verified_facts_are_incomplete`; `tests/provenance.rs::signed_mixed_history_requires_missing_agent_markers_without_marking_human_tests`; `tests/provenance.rs::missing_runs_untrusted_signers_revocation_and_tampering_stay_incomplete`; `tests/git_trailers.rs` | Verified in controlled Git histories. |
| 9. Reproducible results | `tests/cli.rs::trusted_policy_detects_disabled_checks_and_formats_preserve_exit_codes`; `tests/policy.rs::recheck_pins_the_selected_policy_commit_after_a_branch_moves`; `tests/execution.rs::executed_tools_lock_inputs_timestamps_and_distinct_artifacts_are_reported`; `tests/sarif.rs::malformed_partial_or_out_of_snapshot_reports_never_pass_and_keep_raw_evidence` | Verified in controlled repositories. |
| 10. Ecosystem extension | `adapters/syntax/syntax_tests.rs::parses_framework_tests_and_annotations_in_java_python_rust_go_and_typescript`; `adapters/rules_tests.rs::source_patterns_only_report_added_source_lines`; `adapters/structure_rules_tests.rs::configured_import_boundaries_only_match_added_syntax_imports`; `tests/custom_rules.rs::private_rule_runs_on_java_and_python_with_stable_diagnostics_and_snapshot_isolation`, `project_rule_directory_is_loaded_from_selected_policy_snapshot`; `tests/python.rs::real_python_extra_dependency_marker_retention_and_test_repair`; Java/Maven evidence in `tests/maven.rs`; `config_tests.rs::embedded_archive_keeps_the_required_research_lanes`, `builtins_reject_controls_not_declared_by_the_archived_source`, `builtins_reject_language_scopes_that_mismatch_lifecycle_inputs`; `config/discovery/tests.rs::language_scoped_builtins_are_not_advertised_outside_lifecycle_lanes`; `tests/quality/skill_package.rs::executable_catalog_reads_skill_reference_rules_without_compiled_manifests` | Verified for documented syntax, project-local policy rules, Java/Python project adapters, and externally packaged Skill rules. Unsupported semantics remain explicit capability gaps. |

The matrix above is intentionally narrower than a production claim: all
repository-controlled assertions must keep passing, but §9.1.3 still requires
the real-pilot evidence described below.

## Pilot and rollout (§9.2–§10)

`docs/pilot.md` defines the immutable case record, baseline attribution,
reviewer classifications, repair budget, denominators, and the required
measurements. The following evidence is not available in this repository and
cannot be manufactured from fixtures:

1. A team-selected real repository, trusted policy reference, task types,
   historical samples, observation period, sample size, and predeclared
   thresholds.
2. Reviewer-approved classification of unique diagnostics, including existing
   tool attribution and reviewed/unreviewed false-positive counts.
3. Actual agent repair attempts and full rechecks against the selected
   repository, plus comparable review-burden and quick/full cost observations.
4. An authorized rollout decision showing that the M1–M4 scope is adopted only
   after the agreed thresholds are met.

Consequently, §9.2 and the real-repository portions of M1–M4 are **pending**.
The implementation is ready to collect that evidence; it is not represented as
a completed pilot.

## Current repository verification

For commit `aef969a5777d296592c97478e69398003c96841c`, the local mandatory
suite passed formatting, compilation, Clippy with warnings denied, all targets
and features, and LLVM coverage at 94.84% lines. The self-hosted full profile
completed all ten checks, and package compilation succeeded. Remote
[PR Checks run 34746651406](https://github.com/coolplayagent/qualitygate-cli/actions/runs/34746651406)
passed all 20 jobs; [Bazel run 34746651386](https://github.com/coolplayagent/qualitygate-cli/actions/runs/34746651386)
also passed. These runs validate this repository revision only; they do not
replace the pilot evidence above.
