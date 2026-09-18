# Requirements acceptance evidence

## Issue 11 Python and neutral built-ins

| Requirement | Current evidence |
|---|---|
| Six packaged, discoverable and opt-in rules with fixed file/Python scopes and warning default for `no-print` | `tests/issue11_rules.rs::six_opt_in_rules_are_discoverable_and_reject_invalid_scope`; packaged YAML, lifecycle source mappings and release archive checks |
| Added Python file patterns, cross-language Unicode range check, independent warning gate and path override | `tests/issue11_rules.rs::python_and_neutral_file_rules_have_independent_scopes_and_warning_gate`; shared bounded scanner and existing malformed-input/overflow unit tests |
| Added pytest-discoverable `test` prefixes and configurable commit subject format | `tests/issue11_rules.rs::python_naming_detects_all_pytest_prefixes_and_commit_format_is_configurable`; syntax parser and commit evaluator |
| Independent compliant/violating goldens for each rule in minimal, typical and stress suites | `fixtures/{minimal,typical,stress}/issue11-cases.json` and `fixtures/golden/issue11-{minimal,typical,stress}.json`; `tests/selfcheck.rs` |
| Explicit lexical and standards limits | `docs/rules.md`, `skills/qualitygate-cli/references/builtin-rules.md`, lifecycle matrix critical adoption fields |

## Issue 10 native and test file built-ins

| Requirement | Current evidence |
|---|---|
| Three discoverable, opt-in, error-level packages with fixed language scope and configurable test paths | `tests/issue10_rules.rs::three_rules_are_discoverable_and_language_scope_is_fixed`; packaged YAML, lifecycle mappings and release archive checks |
| Added C/C++ unsafe and printf patterns, including headers; modified and unrelated files excluded | `tests/issue10_rules.rs::native_rules_find_added_c_and_cpp_calls_and_ignore_modified_or_other_languages` |
| Language-neutral sleep patterns limited to test paths, with repository path override | `tests/issue10_rules.rs::test_sleep_uses_default_and_configured_test_paths_across_languages` |
| Independent pass/fail goldens for all three rules in minimal, typical and stress suites | `fixtures/{minimal,typical,stress}/issue10-cases.json` and `fixtures/golden/issue10-{minimal,typical,stress}.json`; `tests/selfcheck.rs` |
| Bounded scanner, deterministic output and incomplete selected malformed text or overflow | `adapters::builtin_conventions::tests::native_and_neutral_file_rules_do_not_pass_unreadable_selected_inputs`, `literal_secret_rule_checks_only_added_java_files_and_hides_the_value`; shared `adapters::parallel` worker tests |
| Pure-domain Miri remains complete within CI time budgets | `.github/workflows/pr-checks.yml` partitions `domain::` into four disjoint filters; test-harness `--list` counted 2 + 12 + 21 + 27 = 62, matching the full selector. The prior 30-minute job ended incomplete during a slow interpreted evidence test. |


## Issue 9 reusable built-ins

| Requirement | Current evidence |
|---|---|
| Four discoverable, configurable, opt-in packages with error defaults | `tests/issue9_rules.rs::four_conventions_are_discoverable_and_fail_closed_on_invalid_parameters`; packaged YAML definitions and lifecycle mappings |
| Commit subject, added Java test naming and added-file secret diagnostics with repair and configurable pattern | `tests/issue9_rules.rs::commit_test_name_and_secret_rules_report_violations_then_repairs`, `commit_pattern_override_changes_the_enforced_convention` |
| Annotated added tests require declared and resolved Maven coordinates; missing, stale or ambiguous facts are incomplete; an empty annotation selection passes | `adapters::builtin_conventions::tests::annotation_dependency_selects_added_annotated_tests_and_requires_bound_maven_facts`; `tests/issue9_rules.rs::dependency_rule_without_bound_maven_producer_is_incomplete`, `annotation_dependency_uses_fresh_resolved_maven_project_facts` |
| Bounded large-repository parallelism with deterministic evidence | `adapters::parallel::tests`, `adapters::builtin_conventions::tests::large_unchanged_tree_and_many_java_changes_keep_bounded_analysis`; existing `tests/large_repository.rs` covers snapshot acquisition |


The [phase-A baseline record](pilot-phase-a.md) tracks REQUIREMENTS §12 and
EVO-01/03/05 preparation: versioned Rust task templates, real assertion
regressions, pinned repository tool evidence, and the user-selected Codex
medium/lower model probes. Probe failures and corrected offline interpretations
are retained. These records do not establish cross-product compatibility,
autonomous repair benefit, or acceptance of the real-repository pilot below.

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
| Issue 9 conventions and large-tree performance | `tests/issue9_rules.rs`, `adapters/builtin_conventions_tests.rs`, `adapters/parallel.rs` tests, and each suite's `issue9-cases.json`/`issue9-*.json` pair; CLI violations, incomplete Maven evidence, 18,000-file selection, bounded worker overlap, and deterministic serial/parallel parsing are asserted |
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

1. A sealed concrete task inventory with trusted task/policy references,
   assigned reviewers, monetary cap and durable evidence location. The user
   confirmed qualitygate-cli, eight tasks over seven days, task types, per-run
   budgets and thresholds on 2026-09-16; see the [phase-A record](pilot-phase-a.md).
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

## Rule contracts and paired test effectiveness (§11)

[Required text and minimum entity assertions](custom-rules.md) have separate
empty-scope, text-violation, staged-repair and retained-marker regressions in
`tests/custom_rules.rs`. The four `custom-contract-*` minimal goldens use actual
production evaluation and retain malformed syntax as incomplete.

The [test effectiveness evidence matrix](test-effectiveness.md#requirement-to-test-evidence)
links pure per-file decisions, strict JUnit normalization, immutable overlays,
reusable task/configuration validation and the separate Rust integration target.
Actual fixture assertions establish new-code success and old-code failure;
missing symbols, unknown types, shared-budget timeouts, source mutation and
incomparable identities never supply a counterexample.

The 2026-09-15 Linux verification passed 361 tests, the full 338-fixture
selfcheck, 95.74% line coverage, 21 Miri domain tests, 198 ASan native unit tests
and six affected Bazel owner targets. Nine existing live-producer tests remain
separate. Formatting, compilation, Clippy, documentation and architecture passed;
architecture retained 144 source digests and 1,413 dependency references without
violations. Local evidence is under `target/verification-contract-*`, with
architecture JSON/DOT in `target/architecture/`. See the
[implementation record](implementation.md#rule-text-contracts-and-test-counterexamples-11)
for measured limits; these fixtures do not establish real-pilot or production
producer acceptance.

## Task feedback and external repairs (§12 / phase B)

[Phase-B evidence](pilot-phase-b.md) records the bounded feedback projection,
original/full replay contexts, external Rust harness, real merged-branch
regression, and controlled Codex module repairs with all failed attempts.
These observations establish an engineering loop; the production task inventory,
human benefit, monetary costs and seven-day trial above remain pending.

## Case provenance and pilot aggregation (§12 / phase C)

[Phase-C evidence](pilot-phase-c.md) records suite v2 provenance constraints,
assignment-based metrics, digest-bound full reports, remaining task templates
and an offline replay of every retained phase-B success and failure. The CLI
marks summaries descriptive and cannot authenticate caller declarations or
approve rollout. The production task inventory, reviewer, monetary cap,
durable archive and seven-day observations remain pending.

## Pilot plan integrity (§12 / phase D)

[Phase-D evidence](pilot-phase-d.md) records the `pilot seal` contract and maps
its plan/governance/matrix invariants to native and `pilot_seal` integration
tests. The embedded digest permits later actual-model disclosure and observations
while rejecting changes to the frozen comparison inputs. It is not a signature
or trusted timestamp. The concrete 8-task inventory and external archive record
remain pending, so the production trial has not started.

## Pilot start authorization (§12 / phase E)

[Phase-E evidence](pilot-phase-e.md) records the exact authorization subject,
external trust-store boundary and DSSE/Ed25519 owner verification. Native adapter
tests and the `pilot_authorization` integration target cover plan/repository
binding, human-owner scope, expiry, revocation, absent authorization and
repository-owned trust rejection. No formal owner record or real observation
exists in the repository, so the production trial remains unstarted.

## Independent pilot decision (§12 / phase F)

[Phase-F evidence](pilot-phase-f.md) records the deterministic nine-check
assessment, complete manifest/start-authorization binding and independent
reviewer DSSE decision. Domain, adapter and `pilot_acceptance` integration tests
cover accepted, rejected, failed, unknown, revoked and stale evidence. The tests
use synthetic reports and keys, so no production trial has been accepted.

## Structured pilot budget (§12 / phase G)

[Phase-G evidence](pilot-phase-g.md) records schema-v2 monetary cap sealing,
all-attempt model/infrastructure accounting, priced human review time and the
tenth signed acceptance check. Domain and `pilot_acceptance` CLI tests include
overspend with an otherwise passing relative cost ratio. Actual prices, human
rates, task inventory and seven-day observations remain unavailable, so the
production trial still has no accepted result.

## Stratified pilot inventory (§12 / phase H)

[Phase-H evidence](pilot-phase-h.md) records v3 predeclared task counts, a
distinct-input roster, duplicate task ID/contract rejection and seal-drift
tests. A same-input v2 old/new binary comparison protects existing plan digests.
The task IDs and contract digests do not establish real-world independence;
the production issue/commit roster and seven-day observations remain pending.

## Pilot task source artifacts (§12 / phase I)

[Phase-I evidence](pilot-phase-i.md) records v4 one-to-one source declarations,
bounded archive reads, source-file drift refusal and old v3 seal compatibility.
Domain and `pilot_seal` CLI tests use synthetic files. The team still needs to
confirm actual issue/commit sources and keep the first external archive record
before any seven-day pilot can start.

## Pilot execution order (§12 / phase J)

[Phase-J evidence](pilot-phase-j.md) records v5 predeclared alternating order,
stratum-level counterbalancing and audit of observed start-sequence deviations.
Domain and CLI tests use synthetic positions. The team must retain actual start
events and independently review their order before accepting a production trial.

## Pilot repair stopping (§12 / phase K)

[Phase-K evidence](pilot-phase-k.md) records v6 initial full reports, strict
diagnostic-debt progress and audited retry/time/no-progress stop limits. Domain
and CLI tests use synthetic reports. The team must retain actual Agent and clock
logs and independently review their authenticity before a production decision.

## Pilot model identity (§12 / phase L)

[Phase-L evidence](pilot-phase-l.md) records v7 archived model captures, request
identity and collection-time checks, and explicit unknown routed models. Domain
and CLI tests use synthetic records; the team must compare them with the
provider's original logs and review capture times before a production decision.

## Pilot attempt model identity (§12 / phase M)

[Phase-M evidence](pilot-phase-m.md) records v8 per-attempt captures, including
failed and timed-out runs, and audits declared route drift. These files and
timestamps remain caller claims until an independent reviewer checks original
provider and harness logs.

## Pilot attempt execution timing (§12 / phase N)

[Phase-N evidence](pilot-phase-n.md) records v9 archived harness start/end
times for every attempt and audits declared duration and start sequence.
Domain and CLI tests use synthetic records; the team must compare original
process and clock logs before a production decision.

## Pilot without pricing (§12 / phase O)

[Phase-O evidence](pilot-phase-o.md) records v10 sealing without hourly rates
or prices and an eight-check nonfinancial final assessment. Old priced plans
retain their versioned requirements. These engineering tests do not establish
real-task benefit or supply external human authorization.

[The pilot readiness audit](pilot-readiness.md) tracks the remaining real-task,
governance, observation and independent-review evidence. Engineering regressions
do not close those acceptance items.
