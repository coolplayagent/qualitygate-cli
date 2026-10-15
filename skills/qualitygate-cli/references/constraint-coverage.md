# Source-bound clause coverage

Requires exact runtime capability `constraints.coverage.v1`. Export
`qualitygate schema constraint` and compare it with the packaged
[constraint schema](schemas/constraint.schema.json). The schema describes one
entry in the policy's optional `constraints` array; existing project-rule v1
is unchanged. At most 256 clauses, 64 check references per clause, and 16
references per example polarity are accepted within the 1 MiB policy budget.
This is a development addition after published 0.5.7; its version string alone
does not establish support. Compare capabilities and executable digest.

Read the source and extract clauses before drafting. Obtain each `source` with
`rules source --document AGENTS.md --section Quality --format json`; never
invent its hash. A clause has `schema_version: 1`, a unique `id`, positive
`version`, `source`, nonempty `statement`, explicit `required`, and `realization`.

For supported clauses, realization uses `kind: checks`, `mechanism` (one of
`regex`, `ast`, `operator`, `tool`, `agent_prompt`), nonempty `checks`, and
nonempty `positive_examples` / `negative_examples` reference lists. Checks must
be explicitly configured policy rules or commands. Task-only check IDs are not
accepted. Classification does not create engine capabilities: label only the
actual mechanism implemented by those checks. Example references document the
review cases; they are not evidence that those examples executed in this run.
Review the rule's actual scope and facts; AST is not full compiler semantics.

For unrepresentable clauses use `kind: unsupported`, a concrete `reason`, and
`evidence_owner`. Preserve requiredness; a required unsupported clause prevents
a complete passing gate. Until a host provides behavior enforcement, preserve
Agent Prompt obligations requiring that enforcement as unsupported. A Prompt
label alone cannot prove Agent compliance.

Run read-only `config --show --format json` to validate the inventory and obtain
`constraint_bindings`. These digests bind the clause, source, examples, and
referenced effective check settings/definitions. An authorized adopting policy
may add `adoption` with `id`, `reviewer`, `reference`, `resolution`
(`initial_mapping`, `rule_updated`, `rule_unchanged`), `rationale`, and the exact
`binding_digest`. Do not create a review on another person's behalf or equate
copying the digest with approval. Local adoption records remain assertions of
the selected policy; signed policy adoption uses the existing external approval
workflow. Generation, validation, and execution do not authorize adoption.

After adoption run matching doctor and a final full check. The automatic
`constraint-coverage` check is reserved and cannot be configured or disabled.
Its `metadata.constraint_coverage` contains every declared clause, adoption
binding, source status, execution status, and snapshot/policy/task digests.
Sources are protected from exclusions and retained in signed policy packages.
Changes to clauses or their check definitions invalidate adoption bindings.

Statuses are `unsupported`, `unadopted`, `stale_adoption`, `source_invalid`,
`pending`, `failed`, `verified`, and `not_applicable`. Missing, disabled,
unselected, timed out, or incomplete mapped checks remain `pending`. All mapped
checks are inspected, so one failure cannot hide another check's evidence gap.
Explicit non-applicability is preserved; an empty mapping is invalid.

Required clause failures block even when their referenced check only emits a
warning. Required gaps make the aggregate incomplete. Optional gaps remain
visible without becoming required blockers; they are not verified clauses.
Quick/path results retain their limited scope. `verified` means only that the
declared mapped checks passed for this report's subject and scope. Coverage
does not prove that every sentence was extracted, that the mapping is logically
sufficient, or that an unimplemented host enforcement capability exists.
