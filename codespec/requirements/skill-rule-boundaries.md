# Skill and rule evidence boundaries

This supplement refines the [product contract](qualitygate-cli.md) and
[Skill guidance requirements](skill-guidance.md). It addresses demonstrated
false positives and instructions that overstate scope or replace selected inputs.
The [research record](../../knowledge/best-practices/engineering-standards/guides/skill-rule-boundaries.md)
records counterexamples, sources and limits; the bilingual
[evidence index](../../docs/en/04-contributor-guide/03-requirements-to-test-evidence.md)
maps the requirements to repeatable tests.

| ID | Requirement | Acceptance evidence |
| --- | --- | --- |
| SKILL-BOUNDARY-01 | Version 2 of `py-sql-string-format`, `shell-comparison-spaces`, `shell-assignment-spaces` and `shell-trap-uncapturable` shall distinguish actual expression/operand roles from matching text. SQL placeholders, array/glob brackets, `IFS= read` and digits in trap action text shall not create those findings. Preserve requiredness, severity, explicit enablement and frozen version-1 behavior. | Language syntax counterexamples, `tests/issue12_rules.rs`, `tests/issue16_rules.rs`, and `frozen_v1_review_definitions_keep_their_original_source_pattern_behavior`. |
| SKILL-BOUNDARY-02 | Determine active signed policy, explicit policy reference or selected snapshot policy before deciding whether local initialization can help. `config --show` reports the active signed package when present, otherwise the local candidate; it does not inspect an explicit ref or selected index/commit. Automatically initialize only confirmed, repairable local absence without active/ref authority. Preserve authentication, malformed-input and conflicting-authority failures. | `only_independent_signed_exact_validation_can_activate_a_policy_and_history_is_immutable`; `explicit_policy_reference_never_suggests_local_initialization`; `snapshot_guidance_depends_on_local_policy_and_selection`; `unreadable_or_malformed_configuration_does_not_suggest_init`. |
| SKILL-BOUNDARY-03 | Distinguish full captured file lookup from delivery/path selection of located findings. A CLI full pass shall not be described as proof that all historical files meet per-file limits or all required paths exist. A whole-repository obligation needs an appropriate existing complete-inventory check or an explicit evidence gap; do not invent a CLI repository-scope flag. | `cli_file_contracts_distinguish_complete_lookup_from_delivery_findings` compares CLI and reusable-core outcomes, historical absence/debt, selected deletion/change and path filtering. |
| SKILL-BOUNDARY-04 | Rule authoring shall preserve the effective project-rule directory, including legacy `custom_rules`. Generation into `qualitygate/rules` does not authorize replacing the selected package. Migration must be explicit and preserve existing definitions, enabled IDs, sources and review bindings. | Existing legacy-directory execution and selected-policy snapshot tests in `tests/custom_rules.rs`; independent authoring traces are required to assess the Skill's migration choices. |
| SKILL-BOUNDARY-05 | Modern startup examples may require capabilities; legacy inspection shall retain the selected trusted executable and inspect only supported commands. Missing V3 support is a capability gap, not an instruction to install or replace a runtime. Use an already available compatible runtime within authorization or report the gap. | `tests/quality/skill_startup.rs`, source-bound V3 authoring tests and separate forward traces for legacy/missing-capability requests. Modern-runtime fixtures alone do not establish legacy behavior. |
| SKILL-BOUNDARY-06 | Preserve the caller-selected pilot inventory, task mix and no-progress limit, including their sealed values. Four bug fixes, four refactors and limit 2 are bundled example settings. Apply the declared bounded limit and retain incomplete observations and post-seal mutation rejection. | `v6_preserves_nondefault_sealed_controls_and_uses_the_selected_stop_limit`; existing pilot sealing/authorization/acceptance targets; independent nondefault-pilot forward trace. |
| SKILL-BOUNDARY-07 | Limit the no-model-invocation claim to `check --feedback`. Distinguish its report rendering from separately authorized `agent-run` and judgment provider execution. Feedback, model output and partial pilot evidence shall not replace the original gate or independent acceptance. | `tests/feedback.rs`, `tests/agent_host.rs`, `tests/judgment_provider.rs`, and the nondefault pilot test's incomplete-evidence assertions. |

These changes narrow claimed evidence to what the selected implementation can
establish. They do not lower required checks or severity, adopt policy, authorize
installation, or infer SQL safety, shell correctness or agent compliance.
Unrelated malformed syntax, unavailable facts and exhausted budgets remain
incomplete. The comparison rule's bounded recovery applies only to its documented
missing-spacing shape; it cannot hide another parser error.

Acceptance mappings identify tests and behavioral scenarios, not completed runs.
Retain actual execution results, final snapshot bindings and forward-test traces
separately. Historical version-1 replay is compatibility evidence even when its
old matcher reproduces a false positive corrected in version 2.
