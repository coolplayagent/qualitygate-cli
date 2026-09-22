# Requirement-to-test evidence

[简体中文](../../zh/04-contributor-guide/03-requirements-to-test-evidence.md) · [Volume index](README.md)

The normative requirements live in
[CodeSpec requirements](../../../codespec/requirements/qualitygate-cli.md).
This chapter is the stable evidence index; it replaces date-stamped
implementation ledgers and phase-by-phase progress notes. Test names identify
repeatable repository evidence, not production deployment or pilot acceptance.

| Requirement area | Principal repeatable evidence |
| --- | --- |
| Snapshot selectors, execution, reports, and exit semantics | `tests/cli.rs`, `tests/execution.rs`, `tests/policy.rs`, native domain tests |
| Initialization and capability gaps | `tests/init.rs` |
| Issue 33 review: large-file test overlays, protected capacity authorization, escaped human guidance | `tests/test_effectiveness.rs::raised_file_budget_reaches_both_test_effectiveness_executions`; `tests/policy_validation.rs::protected_file_capacity_is_bounded_authorized_and_used_for_both_policies`; `interfaces::render::tests::preflight_guidance_escapes_controls_without_changing_json_paths` |
| Oversized acquisition stays incomplete and reports actual bytes plus the minimum retry budget | `fixtures/golden/stress.json`: `snapshot-oversized` pins the exact new Issue 33 message; the incomplete outcome and 2 MiB default are unchanged |
| Issue 33 first-run formats, advisory HEAD/worktree preflight, explicit large-file capacity and unchanged policy semantics | `tests/init.rs::first_run_errors_honor_formats_and_show_an_action`, `init_preflights_tracked_ignored_and_untracked_large_files_without_reading_content`, `init_retains_incomplete_preflight_and_cannot_recommend_an_unsupported_budget`; `tests/large_repository.rs::legacy_large_blobs_are_acquired_explicitly_without_hiding_policy_or_source_changes`, `raised_file_budget_preserves_full_snapshot_bytes_and_digest_under_path_filtering`; `snapshot::git::tests::explicitly_permitted_large_blobs_use_nonempty_bounded_batches` |
| Large-repository acquisition and performance bounds | `tests/large_repository.rs`, `tests/benchmarks.rs`, `tests/policy_performance.rs` |
| Built-in and project rules | `tests/issue9_rules.rs` through `tests/issue20_rules.rs`, `tests/custom_rules.rs`, `tests/rule_authoring.rs` |
| Rule categories, mutation, candidates, promotion, and lifecycle | `tests/rule_management.rs`, `tests/policy_categories.rs`, `tests/policy_candidates.rs`, `tests/policy_promotion.rs`, `tests/policy_lifecycle.rs` |
| File contracts and independent test counterexamples | `tests/file_contracts.rs`, `tests/test_effectiveness.rs` |
| Maven, Python, compatibility, coverage, and SARIF adapters | `tests/maven.rs`, `tests/python.rs`, `tests/compatibility.rs`, `tests/coverage.rs`, `tests/sarif.rs` and explicit live variants |
| Diagnostic ratchets | `tests/ratchet.rs`, language/tool-specific ratchet targets, `tests/c_family_ratchet.rs` |
| Tasks, manual acceptance, provenance, and Git trailers | `tests/manual.rs`, `tests/provenance.rs`, `tests/case_provenance.rs`, `tests/git_trailers.rs` |
| Agent feedback and bounded repair loop | `tests/feedback.rs`, `tests/agent_loop.rs`, explicitly invoked repair/live targets |
| Pilot sealing, authorization, evidence, and acceptance | `tests/pilot_*`, `tests/pilot_seal/v8.rs` through `v10.rs`, native pilot domain tests |
| Decision envelopes and optional judgment | `tests/decision_envelope.rs`, `tests/judgment_provider.rs` |
| Architecture, documentation, site, and Skill release | `tests/quality/architecture.rs`, `documentation.rs`, `site.rs`, `skill_package.rs` |

Issue-focused rule tests retain positive, negative, malformed-input, boundary,
and incomplete-evidence cases. Report-adapter tests separately cover parser
normalization and real producer invocation; an unavailable live producer is not
represented as passing integration evidence.

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

Delivery scope and explicit exclusion evidence: `tests/delivery_scope.rs` covers
changed lines versus repository findings, immutable context, large tracked resources,
selected policy provenance, protected exclusions, empty delivery and command failures.
`tests/quality.rs` validates the packaged Skill links and architecture.
`tests/coverage.rs` and live JaCoCo/ESLint acceptance compare delivery denominators
and findings with explicit repository contracts. `tests/policy_validation.rs`
verifies differing exclusions bind both inputs within the two-tree memory budget,
while equal exclusions retain the independently executed parallel checks.
