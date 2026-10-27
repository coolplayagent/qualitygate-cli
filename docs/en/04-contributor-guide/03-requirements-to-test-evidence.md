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
| Initialization and capability gaps | `tests/init.rs` |
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
| OPERATOR-01 strict v2 composition, complete facts, shared budgets and bounded source observations | `tests/operators.rs`; `domain::operator::tests`; `adapters::operators::tests`; existing `tests/rule_authoring.rs` and `tests/custom_rules.rs` preserve v1 |
| OPERATOR-02 | `tests/operators/v3.rs` (via `tests/operators.rs`): named source correspondence, unchanged tests, staged/path scope, source/policy changes, schema/generation compatibility, missing facts and exhaustion; `domain::operator_v3::tests` and `adapters::operators_v3::tests`: lexical typing, projection, set operations, UTF-8, provenance and shared budgets. |
| Built-in and project rules | `tests/issue9_rules.rs` through `tests/issue20_rules.rs`, `tests/custom_rules.rs`, `tests/rule_authoring.rs` |
| Rule categories, mutation, candidates, promotion, and lifecycle | `tests/rule_management.rs`, `tests/policy_categories.rs`, `tests/policy_candidates.rs`, `tests/policy_promotion.rs`, `tests/policy_lifecycle.rs` |
| File contracts and independent test counterexamples | `tests/file_contracts.rs`, `tests/test_effectiveness.rs` |
| Maven, Python, compatibility, coverage, and SARIF adapters | `tests/maven.rs`, `tests/python.rs`, `tests/compatibility.rs`, `tests/coverage.rs`, `tests/sarif.rs` and explicit live variants |
| Diagnostic ratchets | `tests/ratchet.rs`, language/tool-specific ratchet targets, `tests/c_family_ratchet.rs` |

## Agent and task acceptance

| Requirement area | Principal repeatable evidence |
| --- | --- |
| AGENT-01 — Source-bound consuming-agent obligations, scoped Prompt projection, stale/missing evidence and conflict handling | `tests/agent_contracts.rs`; `domain/agent_contract.rs` scope unit test |
| AGENT-HOST-01 — Consuming-agent injection, repair/resume/handoff, unsupported controls, forged reports, policy changes, immutable delivery and canonical output confinement | `tests/agent_host.rs`; `tests/delivery.rs`, including `equivalent_repository_paths_cannot_bypass_delivery_confinement`; `tests/agent_loop.rs`; pure `domain/agent_host_tests.rs` |
| Tasks, manual acceptance, provenance, and Git trailers | `tests/manual.rs`, `tests/provenance.rs`, `tests/case_provenance.rs`, `tests/git_trailers.rs` |
| Agent feedback and bounded repair loop | `tests/feedback.rs`, `tests/agent_loop.rs`, explicitly invoked repair/live targets |
| Pilot sealing, authorization, evidence, and acceptance | `tests/pilot_*`, `tests/pilot_seal/v8.rs` through `v10.rs`, native pilot domain tests |
| Decision envelopes and optional judgment | `tests/decision_envelope.rs`, `tests/judgment_provider.rs` |

## Documentation and distribution

| Requirement area | Principal repeatable evidence |
| --- | --- |
| Architecture, documentation, site, and Skill release | `tests/quality/architecture.rs`, `documentation.rs`, `site.rs`, `skill_package.rs` |

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
| ENV-39-07: packaged workflow, compatibility and native shells | `tests/quality/skill_package.rs` | `pr-checks.yml` full native Windows test command with debug assertions/overflow checks enabled and debug symbols omitted; PowerShell/Git Bash steps with paths containing spaces |

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
