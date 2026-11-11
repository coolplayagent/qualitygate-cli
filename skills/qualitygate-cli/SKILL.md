---
name: qualitygate-cli
description: "Verify implementation, bug-fix and refactor changes with Qualitygate CLI, author source-bound repository rules, and review design contracts with evidence. Also use for rule configuration, runtime verification and selfcheck."
metadata:
  version: "1.2.0"
  homepage: "https://github.com/coolplayagent/qualitygate-cli"
---

# Qualitygate CLI

Use this Skill to select the appropriate evidence and workflow; use the matching
`qualitygate` executable for Schema export, source binding, validation, generation,
planning and checks. An agent's interpretation never replaces a CLI result.
Preserve the user's scope, selected runtime and existing authorization.

## Choose the task before running commands

| Task | Read when applicable | Boundary |
| --- | --- | --- |
| Verify implementation, bug fixes or refactors | [Verification](references/verification.md) | Establish the selected policy, run the final full check and retain its evidence. |
| Inspect help, capabilities or schemas; verify the runtime | [Runtime startup](references/runtime-startup.md), [compatibility](references/runtime-compatibility.md), [selfcheck](references/selfcheck.md) | These operations need no repository policy; do not initialize one for them. |
| Discover or configure rules | [Rule management](references/rule-management.md), [bundled rules](references/builtin-rules.md) | Read-only discovery can precede initialization; policy context requires the selected existing policy. Candidate edits need authorization within the current task. |
| Translate explicit repository constraints into rules | [Rule authoring](references/rule-authoring.md) | Select V1/V2/V3 by supported evidence, export the matching Schema, bind sources and validate before generation. Generation is not adoption. |
| Review responsibility, dependencies, duplicated knowledge or interface behavior | [Design review](references/design-review.md) | Apply Clean Code, Clean Architecture, DRY and SOLID to concrete contracts and counterexamples. Contextual review is not a deterministic gate. |
| Review concurrency, lifetime, bounds, cross-path behavior or tests | [Public review pack](references/review-pack.md) | Select the relevant cards and retain evidence gaps; do not infer safety from text patterns. |

Read only the references needed for the task. Runtime inspection, rule discovery
and review-only requests do not imply a full repository check or policy adoption.
For code changes, the verification workflow remains a prerequisite to completion.

## Execution and authority invariants

Use the existing repository policy and applicable task contract. Never weaken a
required check, fabricate an approval, invent a trusted ref, or suppress a missing
capability to produce a pass. Separately required repository checks still apply.
Resolve findings within the authorized task and recheck after input changes.

Code-task completion requires the final unfiltered `profile: full` report with
`scope: delivery` or `scope: task`, empty `plan.pending_delivery_checks`,
`gate.complete: true`, `gate.decision: pass` and exit code `0`, plus separately
required checks. Quick/path feedback, preflight, schema validity, prompt delivery
and warning triage each provide narrower evidence. Preserve warnings, incomplete
execution and unknowns. Follow [verification](references/verification.md) for the
complete decision and initialization procedure.

When reporting `check` or `selfcheck`, preserve `verification.conclusion`,
`verified_shapes`, `known_limits` and `unverified_assumptions`. A pass concerns
the verified shapes and scope; it is not proof of a defect-free deployment.

For rule authoring, the LLM may draft a candidate but must use the matching CLI's
source and validation results. Read the complete exported Schema and keep
unsupported obligations explicit. Do not extend the finite DSL from memory or
describe a review signal as proof of a semantic defect. Existing authorization
can cover candidate edits; approval and adoption remain separate decisions.

When effective policy declares `agent_obligations`, obtain the current
`agent-contract` projection with the selected role, task and policy before acting.
Apply its prompt to the consuming agent and reload after resume, handoff or
relevant input changes. Read [Agent contracts](references/agent-contracts.md).
Required gaps block readiness; a ready projection proves neither compliance nor
host enforcement. Preserve higher-priority instructions and existing permissions.
Required host controls need an actual supporting host, not an acknowledgement.

## Runtime and path conventions

Resolve the installed Skill and target repository independently:

| Input | Owner and path |
| --- | --- |
| `QUALITYGATE_SKILL_ROOT` | Absolute installation directory containing this `SKILL.md` |
| `QUALITYGATE_BIN` | Explicitly selected trusted executable, otherwise the matching `assets/<platform>-<architecture>/qualitygate[.exe]` or verified published PATH runtime |
| Built-in assets | Installation's `references/rules/`; optional explicit `QUALITYGATE_BUILTIN_RULES_DIR` |
| Schemas | Installation's `references/schemas/`, compared with runtime exports |
| `REPOSITORY_ROOT` | Absolute root of the target repository, when needed |
| `QUALITYGATE_CONFIG` | `qualitygate.yaml`, or the caller's normalized repository-relative `--config` path |
| Project candidates | Repository's `qualitygate/rules/`; discovery honors legacy `custom_rules` |
| Policy history | Repository's `.qualitygate/policy/` |

Use the shell-specific [startup examples](references/runtime-startup.md). Preserve
an explicit executable or asset override and report invalid selections; do not
silently replace them. Choose assets for the actual OS and architecture. Do not
build an untrusted checkout as the runtime. Version equality does not establish
build identity: inspect capabilities, executable digest and matching assets.

Carry the same `--root`, `--config` and selected policy/task inputs through relevant
commands. Reference examples without `--config` assume the default. Bind variables
again across isolated tool calls. `skill:references/rules/...` is provenance, not
a repository-relative path. Built-in YAML is runtime data; project configuration
is the candidate-edit surface. Discovery does not enable enforcement.

## Initialization prerequisite

For repository verification, follow the canonical
[configuration prerequisite](references/verification.md#initialization-prerequisite).
Identify the selected policy authority first. A confirmed missing local candidate
requires ordinary automatic `init` when no active policy or explicit policy ref
is selected. Missing or invalid selected refs and signed policies require their
own diagnosis; local initialization cannot repair them. Keep discovery and
runtime-only tasks outside this prerequisite. Initialization does not authorize
command adoption, rule enabling or policy adoption.

## Additional workflows

| Need | Reference and control surface |
| --- | --- |
| Select snapshots, trim acquisition inputs or preserve trust evidence | [Operations](references/operations.md); snapshot selectors, reviewed exclusions and external evidence |
| Bound instruction size or retain required paths | [File contracts](references/file-contracts.md); `file` with `change: all` |
| Require assertion counterexamples against old code | [Test effectiveness](references/test-effectiveness.md); explicit command/task `test_effectiveness` |
| Prevent analyzer debt growth | [Diagnostic ratchets](references/diagnostic-ratchets.md); report `mode: ratchet` with a fresh base run |
| Compose typed predicates and bounded counts | [V2 operators](references/operators.md); `rules.operators.v2` |
| Relate collections with lexical lambdas and projections | [V3 operators](references/operators-v3.md); `rules.operators.v3` |
| Track declared clauses, adoption, unsupported gaps and snapshot execution | [Constraint coverage](references/constraint-coverage.md); `schema constraint`, automatic `constraint-coverage` check |
| Run the selected Rust Agent host with guarded delivery | [Agent host](references/agent-host.md); `agent-run`, injection receipts, fresh full checks and immutable export |
| Supply bounded repair context | [Agent feedback](references/agent-feedback.md); `check --feedback` and full rechecks |
| Consume versioned decisions or assess warnings | [Decision protocol](references/decision-protocol.md); schemas, optional `--envelope` and advisory judgment |
| Seal or independently accept a declared pilot inventory | [Pilot evidence](references/pilot-evidence.md); signed owner/reviewer records and full-report bindings |

File contracts and diagnostic ratchets are not additional built-in IDs for
`rules enable`. The CLI advertises actual rule definitions and parameter contracts
through `rules list` and `rules describe`; use that inventory rather than treating
this Skill's examples as exhaustive. A missing capability is a compatibility gap.
Neither a prompt's design recommendation nor an enabled pattern signal proves
that a repository satisfies Clean Architecture or SOLID.
