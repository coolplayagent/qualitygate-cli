# Requirements acceptance evidence

[File contracts](file-contracts.md) and
[diagnostic ratchets](diagnostic-ratchets.md) map each new requirement to
separate unit and temporary-repository integration evidence. They extend the
existing rule/report pipeline; full selfcheck remains independent falsification
evidence and policy experience retains its review boundary.

The 2026-09-14 Linux worktree verification for these extensions passed format,
all-target/all-feature compilation, Clippy with warnings denied and all
**342 native tests**, with **9 existing external-producer tests ignored** for
their separate CI jobs. LLVM line coverage passed at **95.35%** (18,133 lines,
843 missed). The full selfcheck passed **249 minimal/typical/stress fixtures**:
“在已验证形态下未发现问题”. Fixture agreement does not establish live producer,
unrepresented framework, runtime configuration or production behavior.

The architecture report retains **135 source digests** and **1,233 file/line
dependency references**, with no violations and every digest matched to the
current source. The domain, configuration, adapter and application Bazel test
targets passed. Separate nightly gates passed **20 pure-domain Miri tests** and
**188 native ASan tests** with leak detection. CodeSpec and Knowledge maps validated.
Local evidence is retained under `target/rule-maintenance/`, including the
original failure during concurrent builds and the unchanged provenance test's successful
isolated recheck. Final stable/coverage suites used bounded build concurrency
and sequential test scheduling; internal concurrency assertions stayed enabled.
These are local development observations, not native Windows/macOS evidence.

This audit distinguishes fixture falsification evidence from team-owned
real-repository observations. Under issue #1's revised §9 contract, agreement
with independent goldens establishes only that no counterexample was found
among the verified shapes. It cannot prove production correctness, pilot
benefit, reviewer agreement or production-policy adoption.

## Functional acceptance (§9.1)

| Item | Reproducible implementation evidence | Acceptance status |
|---|---|---|
| 1. Applicability and errors | `tests/init.rs::generated_rust_candidate_runs_real_tests_then_repairs_and_rejects_zero_tests`, `tests/cli.rs::init_check_repair_and_staged_bytes_form_a_real_cli_loop`, `tests/cli.rs::missing_configuration_and_invalid_selection_return_incomplete`, `config/discovery/tests.rs::malformed_unsupported_and_dynamic_projects_do_not_gain_semantic_capabilities`, and `domain/gate_tests.rs::empty_or_only_skipped_never_claims_validation` | Verified in controlled repositories. Unsupported or incomplete inputs are reported, not passed. |
| 2. Gate completeness | `tests/cli.rs::command_preconditions_timeout_and_prerequisites_remain_incomplete_with_logs`, `required_tool_absence_blocks_even_when_violation_severity_is_warning`, and `zero_test_success_exit_and_stale_reports_cannot_pass`; `tests/execution.rs` covers missing reports, parser/probe failure, timeouts, preconditions, findings exits, and retained evidence; `domain/gate_tests.rs` covers 0/1/2 gate semantics | Verified in controlled repositories. Display filtering cannot alter the computed gate. |
| 3. Repair loop | `tests/init.rs::generated_rust_candidate_runs_real_tests_then_repairs_and_rejects_zero_tests`, `application/report_gate_tests.rs::test_statistics_cannot_claim_success_with_failures_or_missing_counts`, and `tests/cli.rs::zero_test_success_exit_and_stale_reports_cannot_pass` exercise required-test, assertion-failure, and zero-test protections. `tests/cli.rs::init_check_repair_and_staged_bytes_form_a_real_cli_loop` exercises a controlled check/fix/recheck sequence. | Verified for the controlled fixture shapes in revised §9.1.3. Real-pilot repair remains an unmeasured boundary under §9.2. |
| 4. Task acceptance | `tests/cli.rs::task_acceptance_is_merged_and_quick_reports_pending_delivery`, `tests/policy.rs::selected_policy_preserves_the_original_task_plan_after_candidate_deletion_or_corruption`, `tests/policy.rs::recheck_pins_the_selected_policy_commit_after_a_branch_moves`, and `config/plan.rs::reusable_planning_validates_task_contracts_without_requiring_yaml_loading` | Verified in controlled repositories. |
| 5. Increment accuracy | `application/report_gate_tests.rs::changed_lines_filters_old_issues_and_rejects_unlocated_diagnostics`, `new_diagnostics_match_multiplicity_and_survive_line_and_file_moves`, and `affected_scope_includes_unchanged_files_and_requires_explicit_impact_evidence`; `tests/cli.rs::baseline_analysis_filters_old_diagnostics_even_when_their_line_moves`; `snapshot/snapshot_tests.rs::rename_is_not_new_content_and_digest_includes_path_and_mode` | Verified for the documented report adapters and increment modes. Project-specific impact semantics remain declared adapter capabilities, not inferred. |
| 6. Snapshot consistency | `snapshot/snapshot_tests.rs::staged_snapshot_uses_index_bytes_and_worktree_includes_untracked_files`; `tests/execution.rs::mutating_commands_invalidate_evidence_and_cannot_be_repaired_by_later_commands`, `restored_bytes_mode_changes_and_symlinks_do_not_hide_input_mutations`, and `baseline_input_mutation_and_unexpected_exit_codes_cannot_pass_with_valid_reports`; `tests/policy.rs::staged_tasks_and_executable_mode_changes_use_their_selected_inputs` | Verified in controlled repositories. |
| 7. Rule and policy maintenance | `tests/policy.rs::selected_policy_preserves_the_original_task_plan_after_candidate_deletion_or_corruption`; `tests/source_reviews.rs::source_hash_refresh_alone_cannot_substitute_for_a_bound_review`; `tests/source_reviews.rs::review_records_follow_selected_policy_and_staged_source_bytes`; `tests/custom_rules.rs::changed_source_or_definition_invalidates_policy` | Technical detection and blocking are verified. Protected-reference selection and human approval are team-owned deployment evidence. |
| 8. Source-declaration boundary | `adapters/structure_rules_tests.rs::ai_scope_and_trailer_binding_without_verified_facts_are_incomplete`; `tests/provenance.rs::signed_mixed_history_requires_missing_agent_markers_without_marking_human_tests`; `tests/provenance.rs::missing_runs_untrusted_signers_revocation_and_tampering_stay_incomplete`; `tests/git_trailers.rs` | Verified in controlled Git histories. |
| 9. Reproducible results | `tests/cli.rs::trusted_policy_detects_disabled_checks_and_formats_preserve_exit_codes`; `tests/policy.rs::recheck_pins_the_selected_policy_commit_after_a_branch_moves`; `tests/execution.rs::executed_tools_lock_inputs_timestamps_and_distinct_artifacts_are_reported`; `tests/sarif.rs::malformed_partial_or_out_of_snapshot_reports_never_pass_and_keep_raw_evidence` | Verified in controlled repositories. |
| 10. Ecosystem extension | `adapters/syntax/syntax_tests.rs::parses_framework_tests_and_annotations_in_java_python_rust_go_and_typescript`; `adapters/rules_tests.rs::source_patterns_only_report_added_source_lines`; `adapters/structure_rules_tests.rs::configured_import_boundaries_only_match_added_syntax_imports`; `tests/custom_rules.rs::private_rule_runs_on_java_and_python_with_stable_diagnostics_and_snapshot_isolation`, `project_rule_directory_is_loaded_from_selected_policy_snapshot`; `tests/python.rs::real_python_extra_dependency_marker_retention_and_test_repair`; Java/Maven evidence in `tests/maven.rs`; `config_tests.rs::embedded_archive_keeps_the_required_research_lanes`, `builtins_reject_controls_not_declared_by_the_archived_source`, `builtins_reject_language_scopes_that_mismatch_lifecycle_inputs`; `config/discovery/tests.rs::language_scoped_builtins_are_not_advertised_outside_lifecycle_lanes`; `tests/quality/skill_package.rs::executable_catalog_reads_skill_reference_rules_without_compiled_manifests` | Verified for documented syntax, project-local policy rules, Java/Python project adapters, and externally packaged Skill rules. Unsupported semantics remain explicit capability gaps. |

All repository-controlled assertions must keep passing. The matrix makes no
production claim; the real-pilot evidence below remains separate.

## Issue #1 fixture acceptance

| Requirement | Authoritative evidence |
|---|---|
| Minimal/typical/stress and independent goldens for each rule | `fixtures/**`, `config/selfcheck.rs`, `application/selfcheck.rs::validate_coverage`; every catalog rule needs compliant/violating pairs in every suite |
| Full and filtered installed selfcheck | `tests/selfcheck.rs::full_corpus_runs_real_evaluators_and_keeps_negative_cases_green_only_on_golden_agreement`, `filters_are_composable_and_unknown_rules_cannot_pass_empty_regression` |
| Rule weakening must turn regression red | `weakening_commit_rule_is_falsified_with_fixture_assertion_and_input_evidence` changes a copied active rule to accept empty subjects; unchanged goldens require exit 1 |
| Fixture, assertion and input diagnostics; incomplete is distinct | Golden comparison unit test; mutation integration test; `missing_rule_assets_are_structured_incomplete_evidence`; native missing-object, oversized-file, symlink, timeout and overflow cases |
| Report verified shapes, limits and assumptions | `check_reports_preserve_gate_semantics_and_expose_boundaries_in_all_formats`; domain verification boundary and early incomplete reports |
| No candidate command execution during selfcheck | `selfcheck_needs_no_repository_and_never_executes_candidate_commands`; only fixed probes and temporary Git repositories are used |
| Regression found during implementation | Typical Go import golden exposed duplicate declaration/spec facts; `syntax_tests.rs::go_imports_have_one_fact_per_spec_and_preserve_grouped_locations` and `go-grouped-import-delta` retain the repair |
| M1 and CI integration | Revised REQUIREMENTS §9–10, separate PR minimal gate, scheduled full native corpus, full local policy and Skill selfcheck reference |

The full command's JSON is the current corpus evidence. Workflow configuration
alone does not establish that remote OS runs passed. Live producers, actual
human review and production deployments remain outside synthetic fixtures.

## Issue #2 large-repository acceptance

| Requirement | Authoritative evidence |
|---|---|
| Small diff with a tree larger than the old 16 MiB capture limit works for staged, worktree, diff and path | `tests/large_repository.rs::eighteen_thousand_files_with_small_diff_support_all_selectors_and_bounded_parallelism`: 18,000 unique blobs, over 32 MiB, one changed file; complete violation reports rather than acquisition failures |
| Bound parallel content readers and preserve results | Shared acquisition semaphore in `snapshot/limits.rs`, bounded task queues in `git.rs`/`worktree.rs`; the large-repository fixture compares serial/four-reader digests, line maps and messages and enforces 60-second capture / 120-second quick thresholds |
| Preflight budgets and strict protocol validation | `snapshot/git_tests.rs`: batch bytes/count, missing/mismatched objects, malformed sizes/framing and file-count bound; `snapshot_tests.rs::oversized_committed_objects_fail_preflight_and_worktree_limits_are_bounded` |
| Caller budgets, timeouts and incomplete execution | `snapshot_tests.rs::acquisition_budgets_timeout_and_filtered_scope_remain_explicit`, `expired_mapping_and_hashing_deadlines_never_return_partial_evidence` and `git_and_worktree_readers_share_the_acquisition_limit_and_release_on_cancellation`; large-repository CLI rejects a 16 MiB total budget with exit 2; existing runner tests exercise process-tree timeout/cancellation |
| Composable path scope and reproducible repair | `tests/large_repository.rs::path_combines_with_selectors_preserves_bytes_and_recheck_options`; existing policy and execution tests continue to require full-tree policy/snapshot consistency |
| Large rename sets avoid quadratic baseline scans | Digest-indexed removed-file lookup in `snapshot/changes.rs`; `tests/benchmarks.rs::rename_storm_preserves_lines_and_deterministic_old_paths` |

See [the acquisition and performance contract](large-repositories.md). Local
measurements cover the controlled Linux fixture. Native Windows/macOS and the
reporter's actual repository require their own execution evidence.

The 2026-09-14 Linux worktree verification passed format, all-target/all-feature
check, Clippy with warnings denied, and 289 stable tests. The nine external
producer scenarios remain separate ignored tests with dedicated CI jobs.
LLVM line coverage passed at **95.51%** (13,485 lines, 605 missed).
All **249** full selfcheck fixtures agreed with their goldens. The architecture
report covered **102** current source digests and **820** file/line dependency
references with no violations. Snapshot/application/interface Bazel test
targets also passed. Separate nightly checks passed **13** pure-domain tests
under Miri and **167** native unit tests under ASan with leak detection, using
Rust 1.100.0-nightly (4b6d04e70, 2026-09-13). Logs are retained locally under
`target/issue2-*.log`; the selfcheck report is
`target/issue2-selfcheck-final.json`. These are worktree observations, not a
published release or a claim of native Windows/macOS execution.

## Issue #3 rule management acceptance

Issue #4's complete requirements and current implementation evidence are tracked
in [policy evolution](policy-evolution.md). Category, candidate, protected
validation, promotion/rollback, lifecycle and performance suites are separate
integration targets (`policy_categories`, `policy_candidates`,
`policy_validation`, `policy_promotion`, `policy_lifecycle`,
`policy_performance`). Pure oracle/measurement and storage boundary tests stay
in the unit target. [Validation contracts](policy-validation.md) describe the
external authorization boundary and the limits of each reported observation.

See [the command and performance contract](rule-management.md). Evidence is
kept separate from the existing project-rule authoring and snapshot gates.

| Requirement | Authoritative evidence |
|---|---|
| Dynamic defaults, category overview, creation, atomic rename/deletion and forced fallback | `tests/rule_management.rs::progressive_discovery_categories_are_mutable_and_do_not_enable_rules`; `config/categories.rs::registry_bounds_origins_and_assignments_are_strict` |
| Composable category/language/source filters and one assignment per rule | The progressive-discovery integration test covers mixed filters, reassignment, empty/new categories, starter rename/deletion and custom provenance |
| Disable and enable preserve settings, profiles and candidate/trusted-policy separation | `configure_validates_types_merges_dotted_defaults_and_preserves_overrides`; `configure_preserves_deliberately_scoped_profiles_and_validates_required_promotion`; `cli_disabled_rule_stays_blocked_by_selected_trusted_policy`; `category_edits_preserve_reviews_and_executable_edits_expose_stale_bindings`; existing snapshot-policy suites |
| Describe versions, implementations, capabilities, defaults, languages, references and accepted parameters | `every_builtin_parameter_contract_describes_and_validates_its_packaged_default`; project discovery test covers custom DSL descriptions |
| Configure validates parameter names/types/semantics and applies multiple edits atomically | `configure_validates_types_merges_dotted_defaults_and_preserves_overrides`; malformed JSON, bad regex, type errors, unknown keys, overlapping paths and unsupported project parameters preserve original bytes |
| Atomic publication, conflict detection, permission/path confinement and reproducible records | `category_errors_and_lock_conflicts_never_partially_publish`; Unix permission/symlink integration test; `rule_management::tests::optimistic_conflict_preserves_the_other_writers_bytes`; successful mutations return operation and before/after byte digests |
| Bounded parallelism with deterministic results and no partial success | `config/parallel.rs` verifies concurrent overlap, maximum active workers, ordered errors, expiration and worker panic; `project_inventory.rs::full_rule_package_parallel_parsing_matches_serial_and_has_a_time_budget` compares 256 parsed definitions with serial/four-worker execution |
| Large-repository performance requirements | `project_discovery_and_assignment_work_before_activation_with_bounded_large_repo_cost`: 18,000 unrelated files and 256 project rules, five-second per-query thresholds and category JSON below 2 KiB; existing `tests/large_repository.rs` retains snapshot capture/quick-check budgets |

The Rust integration CI/local policy includes the new rule-management suite;
worker and schema tests remain in the separate native unit gate. Timings are
controlled-host observations, not a universal speedup or deployment guarantee.

The 2026-09-14 Linux worktree verification passed `cargo fmt --all -- --check`,
`cargo check --all-targets --all-features`, Clippy with warnings denied, and
`cargo test --all-targets --all-features`: **303 passed**, with the existing
**9 external-producer tests ignored** for their dedicated integration jobs.
`cargo llvm-cov --all-targets --all-features --fail-under-lines 90` passed at
**95.58% Rust lines** (14,270 lines, 631 missed). The complete selfcheck passed
all **249** minimal/typical/stress fixtures against their independent goldens:
“在已验证形态下未发现问题”. Synthetic fixture agreement does not establish live
producer, unrepresented framework, runtime-configuration or deployment behavior.

The architecture report contains **107** current source digests and **837**
file/line dependency references, with no digest mismatch or violation.
The affected configuration/interface Bazel targets passed. Separate nightly
verification passed **13** pure-domain Miri tests and **172** native ASan tests
with leak detection. No native Windows/macOS result is claimed here.
Logs and JSON evidence are retained locally in `target/issue3-*-final.log`,
`target/issue3-miri.log`, `target/issue3-selfcheck-final.json` and
`target/architecture/report.json`. These are local development observations.

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

Release portability is exercised by
`tests/quality/skill_package.rs::skill_frontmatter_accepts_lf_and_crlf_without_accepting_malformed_delimiters`
and the existing complete package contract. The same YAML and metadata checks
apply on Windows and Unix; malformed delimiters remain failures. Live Maven
acceptance retains all three dependency/repair scenarios and prints bounded
fixture-local producer logs on unexpected outcomes, so incomplete execution
remains distinguishable from a rule violation.

For commit `aef969a5777d296592c97478e69398003c96841c`, the local mandatory
suite passed formatting, compilation, Clippy with warnings denied, all targets
and features, and LLVM coverage at 94.84% lines. The self-hosted full profile
completed all ten checks, and package compilation succeeded. Remote
[PR Checks run 34746651406](https://github.com/coolplayagent/qualitygate-cli/actions/runs/34746651406)
passed all 20 jobs; [Bazel run 34746651386](https://github.com/coolplayagent/qualitygate-cli/actions/runs/34746651386)
also passed. These runs validate this repository revision only; they do not
replace the pilot evidence above.
