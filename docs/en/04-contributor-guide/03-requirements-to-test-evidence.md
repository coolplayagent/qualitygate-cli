# Requirement-to-test evidence

[简体中文](../../zh/04-contributor-guide/03-requirements-to-test-evidence.md) · [Volume index](README.md)

The normative requirements live in
[CodeSpec requirements](../../../codespec/requirements/qualitygate-cli.md).
This chapter is the stable evidence index; it replaces date-stamped
implementation ledgers and phase-by-phase progress notes. Test names identify
repeatable repository evidence, not production deployment or pilot acceptance.

## Snapshots, initialization and acquisition

| Requirement area | Principal repeatable evidence |
| --- | --- |
| Snapshot selectors, execution, reports, and exit semantics | `tests/cli.rs`, `tests/execution.rs`, `tests/policy.rs`, native domain tests |
| SNAP-RENAME-01: deterministic one-to-one rename sources, surplus additions and unmatched deletions | `snapshot::tests::rename_sources_are_consumed_once_in_path_order`: text/binary/empty content, retained originals and modes; `tests/delivery_scope.rs::surplus_rename_copies_remain_added_and_subject_to_delivery_rules`: worktree/staged/path/diff findings and repair; `tests/benchmarks.rs::duplicate_content_rename_storm_consumes_each_old_path_once`: 18,000 identical sources plus an added copy; existing acquisition timeout tests remain required |
| Initialization and capability gaps | `tests/init.rs` |
| CONFIG-FILE-01: bounded regular configuration/discovery inputs, preserved missing-policy semantics and no publication after invalid input | `config::tests::configuration_file_size_budget_is_inclusive_and_precedes_parsing`; `tests/init.rs::fifo_configuration_is_rejected_without_initialization_or_replacement`, `fifo_ignore_file_blocks_discovery_before_candidate_publication` (Unix); existing unreadable/malformed-policy regressions |
| RULE-ASSET-01: aggregate actual-byte accounting, inclusive byte/file limits and regular asset targets | `config::catalog_assets::tests`: 1 MiB/256 boundaries, stable order, regular/special symlink targets and zero-length metadata; `tests/selfcheck.rs::underestimated_asset_lengths_are_incomplete_before_parsing_and_recover_after_repair`: Linux procfs bytes exceed the allowance before parsing, structured incomplete selfcheck and restored rule/corpus identity |
| Large-file test overlays, protected capacity authorization, escaped human guidance | `tests/test_effectiveness.rs::raised_file_budget_reaches_both_test_effectiveness_executions`; `tests/policy_validation.rs::protected_file_capacity_is_bounded_authorized_and_used_for_both_policies`; `interfaces::render::tests::preflight_guidance_escapes_controls_without_changing_json_paths` |
| Oversized acquisition stays incomplete and reports actual bytes plus the minimum retry budget | `fixtures/golden/stress.json`: `snapshot-oversized` pins the acquisition diagnostic; the incomplete outcome and 2 MiB default are unchanged |
| First-run formats, advisory HEAD/worktree preflight, explicit large-file capacity and unchanged policy semantics | `tests/init.rs::first_run_errors_honor_formats_and_show_an_action`, `init_preflights_tracked_ignored_and_untracked_large_files_without_reading_content`, `init_retains_incomplete_preflight_and_cannot_recommend_an_unsupported_budget`; `tests/large_repository.rs::legacy_large_blobs_are_acquired_explicitly_without_hiding_policy_or_source_changes`, `raised_file_budget_preserves_full_snapshot_bytes_and_digest_under_path_filtering`; `snapshot::git::tests::explicitly_permitted_large_blobs_use_nonempty_bounded_batches` |
| Missing-policy guidance, literal command arguments, explicit policy references and safe Windows path display | `tests/init.rs::first_run_errors_honor_formats_and_show_an_action`, `missing_custom_configuration_is_carried_into_the_command` (executes returned arguments with spaces/apostrophes), `explicit_policy_reference_never_suggests_local_initialization`, `snapshot_guidance_depends_on_local_policy_and_selection`, `unreadable_or_malformed_configuration_does_not_suggest_init`; `paths::tests::displays_ordinary_windows_paths_without_verbatim_prefix` (Windows) |
| DIST-04 Skill configuration prerequisite, automatic init, failure stop and post-init recheck | `tests/quality/skill_package.rs::skill_initialization_contract_requires_auto_init_stop_and_recheck` checks entry metadata, read-only bootstrap examples, initialization boundaries and operational instructions |
| Large-repository acquisition and performance bounds | `tests/large_repository.rs`, `tests/benchmarks.rs`, `tests/policy_performance.rs` |

## Rules, policy and tool evidence

| Requirement area | Principal repeatable evidence |
| --- | --- |
| CONSTRAINT-01 source-bound inventory, adoption, snapshot coverage and explicit gaps | `tests/constraint_coverage.rs`; `domain::constraint::tests` |
| RULE-OUTPUT-01: bounded diagnostic records, serialized bytes and replay expansion | `adapters::diagnostic_budget::tests`; `tests/custom_rules/output_budget.rs`: v1/v2/v3 incompleteness, retained findings, expression/fixed allowance and repair |
| OPERATOR-01 strict v2 composition, complete facts, shared budgets and bounded source observations | `tests/operators.rs`; `domain::operator::tests`; `adapters::operators::tests`; existing `tests/rule_authoring.rs` and `tests/custom_rules.rs` preserve v1 |
| OPERATOR-SET-01: bounded indexed set operations, first origins/order, exact Unicode equality, duplicates and complete empty operands | `tests/operators/v3.rs::v3_file_correspondence_scales_without_relaxing_shared_budgets`; `adapters::operators_v3::tests`: membership oracle, stable observations, 20,000 duplicate scalars, small left/large right subsets, unused-index avoidance and index work/scan exhaustion |
| OPERATOR-02 | `tests/operators/v3.rs` (via `tests/operators.rs`): named source correspondence, unchanged tests, staged/path scope, source/policy changes, schema/generation compatibility, missing facts and exhaustion; `domain::operator_v3::tests` and `adapters::operators_v3::tests`: lexical typing, projection, set operations, UTF-8, provenance and shared budgets; `adapters::custom_rules::selection::tests`: shared acquisition limits, reservations before copying and inherited syntax deadlines; `v3_acquisition_exhaustion_keeps_prior_violations_and_never_claims_complete_inputs`: incomplete acquisition retains fixed findings. |
| Built-in and project rules | `tests/issue9_rules.rs` through `tests/issue20_rules.rs`, `tests/custom_rules.rs`, `tests/rule_authoring.rs` |
| OPERATOR-PORTABLE-01: Python/TypeScript file correspondence without language runtimes | `tests/operators/portable.rs::python_and_typescript_lambdas_need_only_the_cli_and_git` (Unix, via `tests/operators.rs`): Git-only child `PATH`, runtime absence, schema validation, unchanged counterparts, violations, snapshot changes and stale source bindings; this does not execute project tests or prove Windows runtime isolation |
| Rule categories, mutation, candidates, promotion, and lifecycle | `tests/rule_management.rs`, `tests/policy_categories.rs`, `tests/policy_candidates.rs`, `tests/policy_promotion.rs`, `tests/policy_lifecycle.rs` |
| File contracts and independent test counterexamples | `tests/file_contracts.rs`, `tests/test_effectiveness.rs` |
| Maven, Python, compatibility, coverage, and SARIF adapters | `tests/maven.rs`, `tests/python.rs`, `tests/compatibility.rs`, `tests/coverage.rs`, `tests/sarif.rs` and explicit live variants |
| PYTHON-METADATA-01: regular installed metadata, bounded file reads and incomplete producer evidence | `application::python_install::tests`: exact 2 MiB boundary, oversized files, directories and symlink confinement; `tests/execution/python_metadata.rs`: bounded child CLI with FIFO/socket/directory/oversized producer outputs, retained artifacts, blocked dependents and regular-file recovery; live pip remains in `tests/python.rs` |
| REPORT-COUNT-01: checked totals across test and coverage reports, retained evidence and blocked dependents | `application::report_gate::tests::matched_entity_totals_reject_overflow_across_test_and_coverage_reports`; `tests/execution/report_counts.rs` |
| COVERAGE-THRESHOLD-01: exact count decisions, inclusive decimal thresholds, optional branches and empty inventories | `domain::coverage::tests`; `tests/coverage.rs::coverage_thresholds_use_exact_counts_before_rounding_percentages` |
| LCOV-SUMMARY-01: source-specific declared/observed counter diagnostics, retained raw evidence and incomplete verdicts | `adapters::reports::tests::lcov_sections_merge_by_identity_and_validate_source_summaries` covers all four counters after a valid source section; `tests/coverage_llvm_live.rs::rust_lcov_reports_require_measured_branches_and_consistent_source_summaries` uses the pinned LLVM exporter in the separate `coverage-producers` job. |
| OUTPUT-INPUT-01: reject normalized report/project input collisions before preparation, static preflight, baseline protection, fresh reports and producer launch integrity | `tests/execution/preparation.rs`; stale-report regressions in `tests/cli.rs` and `tests/execution.rs` |
| OUTPUT-PATH-01: canonical output-name uniqueness, separate baseline inventory, preserved aliases and no duplicate report counting | `config::tests::report_output_names_are_unique_after_normalization_without_rewriting_policy`; adjacent baseline/project unit tests; `tests/execution/output_aliases.rs` |
| SHARED-EXECUTION-01: base-derived scope binding, shared immutable inputs/digests, isolated workspaces and replacement detection | `snapshot::tests::derived_baseline_preserves_binding_and_isolates_shared_execution_inputs`: base/current digest binding, rejection of equal-length content changes during guard creation and permanent invalidation; `snapshot::input_guard::tests`; `tests/ratchet.rs`; `tests/compatibility.rs` |
| Diagnostic ratchets | `tests/ratchet.rs`, language/tool-specific ratchet targets, `tests/c_family_ratchet.rs` |
| INPUT-PROTECTION-01: Windows shared readers, rejected in-place writes and replacements, permanent attribute invalidation, bounded handles and release on constructor failure/drop | `snapshot::input_guard::tests`: restored in-place write counterexample, sharing violations, overlapping guards, existing writers and failed initial inspection; `snapshot::tests`; `application::doctor_context::tests::exhausted_shared_budget_and_invalid_guard_never_start_another_probe`; `tests/selfcheck.rs`; `.github/workflows/windows-snapshot.yml` |
| INPUT-PROTECTION-01: producer refusal preserves verified inputs, nonzero exit and raw sharing-error evidence; actual attribute mutation invalidates coverage, baseline and paired test executions | `tests/coverage.rs::native_coverage_repairs_preserve_raw_reports_and_reject_inconsistent_or_absent_evidence`; `tests/ratchet.rs::ratchet_baseline_failures_timeouts_mutation_and_tool_mismatch_are_incomplete`; `tests/test_effectiveness.rs::execution_gaps_and_non_assertion_failures_never_establish_effectiveness` |

## Agent and task acceptance

| Requirement area | Principal repeatable evidence |
| --- | --- |
| AGENT-01 — Source-bound consuming-agent obligations, scoped Prompt projection, stale/missing evidence and conflict handling | `tests/agent_contracts.rs`; `domain/agent_contract.rs` scope unit test |
| AGENT-HOST-01 — Consuming-agent injection, repair/resume/handoff, unsupported controls, forged reports, policy changes, immutable delivery and canonical output confinement | `tests/agent_host.rs`; `tests/delivery.rs`, including `equivalent_repository_paths_cannot_bypass_delivery_confinement`; `tests/agent_loop.rs` verifies the repair fixture consumed the complete prompt; `runner::tests::closed_stdin_is_incomplete_even_when_the_child_exits_successfully` preserves failed input delivery; pure `domain/agent_host_tests.rs` |
| Tasks, manual acceptance, provenance, and Git trailers | `tests/manual.rs`, `tests/provenance.rs`, `tests/case_provenance.rs`, `tests/git_trailers.rs` |
| Agent feedback and bounded repair loop | `tests/feedback.rs`, `tests/agent_loop.rs`, explicitly invoked repair/live targets |
| FEEDBACK-WORK-01: deferred diagnostic JSON construction with preserved greedy selection | `domain::feedback::tests::smaller_later_members_and_checks_survive_an_exhausted_inventory`: skip an oversized member, retain a smaller later one and a shorter later check, keep independent timeout evidence and exact omission counts; existing feedback unit/integration tests preserve UTF-8 truncation, report pointers, commands, filtering and gate authority. Performance comparisons require source-bound measurements separate from semantic regression tests. |
| Pilot sealing, authorization, evidence, and acceptance | `tests/pilot_*`, `tests/pilot_seal/v8.rs` through `v10.rs`, native pilot domain tests |
| Decision envelopes and optional judgment | `tests/decision_envelope.rs`, `tests/judgment_provider.rs` |

## Documentation and distribution

| Requirement area | Principal repeatable evidence |
| --- | --- |
| TEST-ISOLATION-01: fixture Cargo outputs cannot share the parent build cache | `tests/pilot_baseline.rs::cargo_fixture_builds_ignore_parent_target_directory` replays real defect/repair/refactor assertions and verifies the parent cache remains untouched |
| Architecture, documentation, site, and Skill release | `tests/quality/architecture.rs`, `documentation.rs`, `site.rs`, `skill_package.rs` |
| CI-NODE-01: Node 24 action runtime with explicit npm cache opt-out | `eslint-project` in `.github/workflows/pr-checks.yml` runs the real ESLint ratchet and repair test; `.github/workflows/release.yml` uses the same setup inputs. `tests/quality/skill_package.rs` validates release packaging; it does not execute publication. |
| CI-WINDOWS-01: exhaustive Windows target partition within unchanged job budgets | `.github/workflows/pr-checks.yml` runs core targets and complementary integration globs in separate native jobs; compare per-target `cargo test ... -- --list` inventories with `--all-targets`, including ignored tests. All three jobs and PowerShell/Git Bash preflights must pass; cancellation remains incomplete. |
| CI-ARTIFACT-01: Node 24 evidence uploads and static Pages delivery | `tests/quality/site.rs::pages_workflow_deploys_only_the_static_site_from_main` binds the site path and hidden-file exclusion; `tests/quality/skill_package.rs` checks packaging. Actual architecture/coverage uploads, three-platform `.github/workflows/selfcheck.yml` artifacts and `.github/workflows/pages.yml` deployment require separate execution evidence. |
| CI-INVENTORY-01: integration target discovery follows Cargo | `tests/quality/gates.rs::documentation_repository_integration_gate_covers_cargo_targets` compares the repository policy with bounded, offline Cargo metadata and rejects target omissions or filtering. `qualitygate.yaml` and the CI `rust-integration` job use a literal `*` target selector; real execution and `--list` inventories verify the selected tests. Ignored tests require their documented environments and separate execution. |

Rule regression tests retain positive, negative, malformed-input, boundary,
and incomplete-evidence cases. Report-adapter tests separately cover parser
normalization and real producer invocation; an unavailable live producer is not
represented as passing integration evidence.

## Evidence interpretation

Pilot fixtures prove the machine contracts for plan sealing, source artifacts,
run order, stopping rules, per-attempt model/execution records, old-schema
compatibility, and v10 nonfinancial thresholds. They do not supply the eight
real tasks, external owner/reviewer signatures, observation window, original
provider logs, or human benefit measurements.

For a change, link each requirement to the narrowest pure/unit test and an
integration counterexample where appropriate. Record unsupported platforms,
unavailable tools, performance environment, and all incomplete results. The
current Git commit and gate report—not a historical prose status line—identify
what was actually verified.

## Delivery scope and evidence identity

| Behavior | Repeatable evidence |
| --- | --- |
| Selector-derived delivery, immutable context, exclusions, empty delivery and unlocated command failures | `tests/delivery_scope.rs`; selector recheck argv preserves the verification digest and rejects the removed scope option. |
| Separate inputs for differing policy exclusions, within the two-tree budget | `tests/policy_validation.rs`; equal exclusions retain independently executed parallel checks. |
| Content digests cannot replace scope-bound verification identities | `tests/decision_envelope.rs`; round-trips check/feedback on every platform and distinguishes two path filters over identical bytes. |
| Effective/configured coverage modes and empty denominators | `tests/coverage.rs`; explicit live JaCoCo/ESLint comparisons retain delivery versus reusable-core repository contracts. |
| No-production-change applicability uses retained context | `tests/test_effectiveness.rs`. |
| Historical versus changed findings, retained multiplicity in either producer order and producer errors | `tests/sarif.rs`, `tests/execution.rs`, `application::report_gate::tests`. |
| Unavailable repository replay is omitted while delivery recheck remains executable | Feedback unit tests and `tests/decision_envelope.rs`. |

## Runtime and preflight

| Requirement | Unit / pure evidence | Integration / platform evidence |
| --- | --- | --- |
| ENV-39-01/02: offline fingerprint, requirements and parser diagnostics | `config::runtime::tests`, `domain::doctor::tests` | `tests/doctor.rs`: offline capabilities/schema and compatibility counterexamples |
| ENV-39-03: exact plan and environment requirements | `config::plan::tests` | `tests/doctor.rs`: dependency expansion, task environment, missing/empty values, staged/diff/policy selection; `tests/policy_promotion.rs`: active signed policy |
| ENV-39-04: static execution boundary and bounded probes | `application::doctor_context::tests` | `tests/doctor/probes.rs`: timeout, overflow, empty version, changed script/input/policy and secret output |
| ENV-39-05: exact per-side budgets | `snapshot::selected_preflight::tests` | `tests/doctor/budgets.rs`: base-only oversize, 2/8 MiB, above maximum, total bytes, protected exclusions, index and truncation |
| ENV-39-06: preflight protocol and executable next steps | `domain::doctor::tests` | `tests/doctor.rs` schema validation and replay of suggested argv |
| ENV-39-07: packaged workflow, compatibility and native shells | `tests/quality/skill_package.rs` | `pr-checks.yml` complete native Windows target partition with debug assertions/overflow checks enabled and debug symbols omitted; PowerShell/Git Bash steps with paths containing spaces |
| EXECUTABLE-FILE-01: regular-file identity before open, retained size/digest checks and symlink compatibility | `runner::identity::tests` | `tests/agent_host.rs::fifo_evaluator_is_rejected_before_opening_or_launching_the_agent` (Unix): bounded old-code counterexample, incomplete ledger, no Agent launch or delivery |

Run `cargo test --test doctor --all-features` independently of native unit tests.
The Windows CI result, full selfcheck, architecture source/line digest evidence,
Cargo/Bazel checks and >=90% coverage remain separately required. Miri exercises
pure domain; ASan exercises native unit tests. A Linux pass does not establish
Windows shell acceptance. See [runtime/preflight](../02-reference/06-runtime-and-preflight.md).

---

[Previous: Selfcheck and fixtures](02-selfcheck-and-fixtures.md) · [Contents](../README.md) · [Next: Pilot evaluation](04-pilot-evaluation.md)

## RESOURCE-43: Default document and media exclusions

| Requirement | Evidence |
| --- | --- |
| Versioned suffix classification, precedence and bounded reason counts | `domain::resource_exclusions::tests` |
| Reuse acquired listings, bound the combined inventory, and omit unselected intermediate HEAD entries | `snapshot::limits::tests::shared_exclusion_inventory_bounds_the_union_and_ignores_bootstrap_omissions`; `tests/delivery_scope/resources.rs::resource_inventory_uses_acquired_trees_without_unselected_head_entries` |
| Exclude before per-file/aggregate budgets; base-only, changed, renamed, deleted, staged and diff resources | `tests/delivery_scope/resources.rs::default_resources_are_excluded_before_budgets_in_every_local_selector`, `aggregate_resources_do_not_consume_content_budget_and_unknown_types_still_fail` |
| Retention, opt-out, protected inputs, explicit exclusion conflicts, strict policy and selected policy | `tests/delivery_scope/resources.rs::retain_disable_and_protected_inputs_restore_acquisition_without_bypassing_limits`, `policy_selection_and_strict_configuration_apply_to_default_exclusions` |
| Missing command evidence stays incomplete; retained and declared tool dependencies remain available | `tests/delivery_scope/resources.rs::command_dependencies_need_retention_and_missing_evidence_stays_incomplete` |
| Policy/inventory identity, history consistency and timeout | `tests/delivery_scope/resources.rs::resource_policy_and_inventory_bind_snapshot_identity_and_history` |
| MR resources do not hide source violations | `tests/merge_request.rs::merge_request_excludes_large_documents_from_source_without_hiding_source_findings` |
| Bootstrap/custom source protection and retained resource read failures | `tests/custom_rules.rs::custom_rule_sources_are_protected_even_with_a_document_suffix`; `tests/delivery_scope/resources.rs::bootstrap_protects_configuration_with_a_resource_suffix`, `restored_resources_do_not_bypass_confined_path_failures` (Unix) |
| Complete counts with bounded details and restored over-budget resources | `snapshot::selected_preflight::tests::resource_counts_and_protected_overrides_survive_detail_truncation` |
