---
name: qualitygate-cli
description: "Use Qualitygate CLI for implementation, bug-fix and refactor verification: run a full snapshot-bound check, repair violations or incomplete evidence, and recheck the final snapshot. If configuration is missing, run init automatically without asking and stop verification until it succeeds. Also translate explicit AGENTS.md constraints into source-bound, schema-validated project-rule candidates through the matching CLI; supports rule configuration, file contracts, diagnostic ratchets and selfcheck."
metadata:
  version: "1.0.2"
  homepage: "https://github.com/coolplayagent/qualitygate-cli"
---

# Qualitygate CLI

Use this Skill as the decision and safety layer over the published
`qualitygate` executable. The Skill routes intent and preserves authority
boundaries; the matching CLI is the executable source of truth for Schema
export, source binding, validation, generation, planning, and checks. Never
replace a CLI result with an LLM's interpretation or an unvalidated equivalent.

This Skill has two primary workflows:

- verify a selected repository snapshot against existing policy and task
  contracts;
- translate explicit repository constraints, including applicable sections of
  `AGENTS.md`, into source-bound, Schema-validated project-rule candidates.

Neither workflow replaces repository policy, task acceptance, review, or
signed evidence with an agent judgment.

For implementation, bug fixes, and refactors, use the existing repository gate
to verify the final code snapshot before reporting the task as complete.
The gate's coverage and evidence determine what that claim means.

When effective policy declares `agent_obligations`, use the
[consuming-agent contract workflow](references/agent-contracts.md) before acting.
Obtain a current `agent-contract` projection with the selected role, task and
policy, and apply its prompt to the agent performing the work. Reload it after
resume or handoff. Required gaps block readiness; a ready projection proves
neither compliance nor host enforcement. Preserve higher-priority instructions
and existing permissions. A required host control needs an actual supporting
host; do not substitute an acknowledgement or code-gate pass.
When the user selects the Rust harness, follow the
[Agent host workflow](references/agent-host.md): `agent-run` injects current
obligations and guards its delivery entry point. Completion identifies a verified
content-addressed artifact; it does not authorize arbitrary live workspace bytes.

## Use the Skill over the CLI

For ordinary code work, let this Skill select the snapshot, profile, evidence,
and recheck workflow, then use the CLI to execute it. For rule authoring, let
the Skill classify each prose obligation by evidence strength, then use
`rules schema`, `rules source`, `rules validate`, and `rules generate` from the
matching CLI. The LLM may draft a candidate; it may not invent a source digest,
extend the finite DSL, declare validation successful, enable the rule, or treat
generation as approval.

Do not hand-write a project rule from memory and skip the CLI because its shape
looks plausible. Read the complete packaged Schema and
[rule-authoring workflow](references/rule-authoring.md), preserve unsupported
obligations as explicit gaps, and route graph/type/data-flow/runtime semantics
to existing lints, project adapters, bounded command checks, or manual review.
Only write or adopt a rule when the user authorizes that policy mutation.

Resolve the runtime → compare version and capabilities → establish configuration
existence and compatibility → run the matching doctor → run formal check.
The [runtime compatibility workflow](references/runtime-compatibility.md) contains
verified introduction versions, old-runtime handling, schemas and a large-repository
template. The initialization prerequisite below applies after runtime resolution;
capabilities and schema export do not require repository configuration.

## Path conventions

Resolve the installed Skill directory and the target repository independently.
`QUALITYGATE_SKILL_ROOT` is the absolute directory containing this `SKILL.md`;
`REPOSITORY_ROOT` is the absolute root of the repository being checked.

| Input | Path and owner |
|---|---|
| Runtime | Skill: `assets/<platform>-<architecture>/qualitygate[.exe]` |
| Built-in rule assets | Skill: `references/rules/`; optional override `QUALITYGATE_BUILTIN_RULES_DIR` |
| Packaged schemas | Skill: `references/schemas/` |
| Candidate configuration | Repository: `qualitygate.yaml`, or the caller's `--config` path |
| Project rule discovery and generation | Repository: `qualitygate/rules/`; discovery honors explicit `custom_rules` |
| Policy history | Repository: `.qualitygate/policy/` |

The `skill:references/rules/...` catalog origin is a provenance label, not a
repository-relative filename. Built-in YAML assets belong to the installation;
project candidates belong to the target repository. `rules generate` always
writes to `qualitygate/rules`, even when discovery uses a legacy `custom_rules`
directory. Discovery alone does not select project rules for enforcement.

Use `QUALITYGATE_CONFIG=qualitygate.yaml` unless the caller selected another
normalized repository-relative path. Carry the same `--root` and `--config`
through `config`, `init`, rule queries, `doctor` and `check`. Examples below use
the resolved absolute `QUALITYGATE_BIN`; reference examples that omit `--config`
assume the default. Shell variables and functions may not survive separate tool
calls: repeat the bindings or substitute the recorded absolute paths each time.

## Initialization prerequisite

Before repository verification, establish whether the required policy exists
for the repository and selected snapshot. Use file/Git inspection or a read-only
`config --show` for this decision. Run this prerequisite separately; do not batch
it with subsequent rule discovery or checks.

When the required repository configuration is missing, run ordinary `init`
automatically without asking the user. Report the repository, exact command and
candidate configuration path, preserving any explicitly selected configuration
path. Automatic initialization does not authorize `--with-checks`, rule enabling
or policy adoption.

Until required `init` has run successfully and the required policy is available,
stop this Qualitygate workflow: do not continue with rule discovery, planning,
checks or policy mutations. Do not substitute repository tests or another review
workflow unless the user separately requests them. If `init` cannot run, fails,
or leaves the required configuration unavailable, report the initialization
prerequisite and leave verification pending. Recheck configuration availability
after initialization before proceeding.

An existing valid policy does not require another `init`. Report malformed or
unreadable configuration as an error; do not overwrite it or treat any failed
configuration inspection as proof that initialization is needed.
For staged/diff/MR checks, a newly initialized worktree policy must also be
present in the selected index/commit before checking. Do not silently stage,
commit or change the snapshot selector to satisfy this prerequisite.

## Resolve the executable

Follow the [runtime startup examples](references/runtime-startup.md) for the
active operating system and shell. Select Linux assets for Linux and WSL,
Windows assets for PowerShell or native Git Bash/MSYS2; choose x86_64 or
aarch64 by architecture. There is no bundled macOS runtime. A missing or
unusable matching asset may fall back to an already verified published
`qualitygate` executable on PATH. Do not silently build an
untrusted repository checkout as the skill runtime.

An intact release archive automatically finds its own `references/rules`.
`QUALITYGATE_BUILTIN_RULES_DIR` is optional; preserve an explicit override and
verify that it belongs to the selected runtime. For PATH fallback or a custom
layout, bind the matching Skill's rule assets explicitly when no override was
provided. An invalid explicit override is an error, not a reason to fall back.

The startup examples record the executable, verify `--version` and capabilities,
and inspect configuration read-only. Apply the initialization prerequisite
before rule discovery, doctor or checks in the repository-verification workflow.
`--version`, help, capabilities, schema export and `selfcheck` need no repository
configuration; do not initialize a repository just to use them.

## Verify code tasks before completion

Compare runtime `--version` with this Skill's metadata and consult
[runtime compatibility](references/runtime-compatibility.md). Query capabilities
and compare exact IDs and executable digest; identical version strings do not
prove identical builds. After configuration inspection, run `doctor` with the
intended selector/profile/task/policy/budgets. Default doctor is static; explicitly
select `--probe-tools` when declared tool execution is authorized. Repair its
structured prerequisites before formal checks. MR doctor is not supported yet;
retain that preflight gap when using the formal MR workflow.

When the runtime advertises `doctor.summary.v1`, prefer `doctor --summary
--format json` with the intended snapshot options. The bounded summary identifies
the complete JSON report in a unique temporary directory through `report.path`,
SHA-256 and byte count. Read that file for full diagnostics and next-step argv;
do not mistake `details_truncated` for evidence loss in the saved report or a
preflight pass for delivery acceptance. Exit 2 remains blocked/incomplete,
including `report.write_failed`. Retain the file until diagnosis is complete.

Use the repository's current policy and any applicable task contract. Inspect
the effective configuration and identify the snapshot being delivered. For
uncommitted edits, `--worktree` captures the current files; use `--staged`,
`--diff`, or `--mr` only when that is the intended delivery scope. Ensure the
repository's required build, tests, and analysis run, whether the policy
selects them or they are required separately. The CLI report describes only
checks selected by the policy.

Use `--profile quick` or `--path` for feedback while editing. After the last
change to checked inputs, run an unfiltered `check --profile full` against the
final snapshot, passing a task contract and trusted policy/evidence inputs when
the workflow requires them. Read [operations](references/operations.md) for
snapshot, task, and trust handling. Mark the code task complete only when
the final report has `profile: full`, `scope: delivery` or `scope: task`
(never `scope: path`), an empty `plan.pending_delivery_checks`,
`gate.complete: true`, and `gate.decision: pass`, with exit code `0`, and all
separately required repository checks pass. Retain its snapshot and policy
digests, and rerun if the checked inputs change. Inspect `selection.mode`,
`selection.exclude`, `selection.excluded_paths`, and `selection.empty_delivery`:
the CLI derives delivery from the selected worktree, index, diff or MR. An empty
delivery is not evidence that all repository code is clean. There is no separate
scope selector; the profile only selects which checks run.

Resolve violations or missing evidence within the user's authorized scope and
recheck. Never weaken a required check or invent policy, task acceptance, or
manual approval to produce a pass. If the CLI, policy, required task evidence,
or project tools are unavailable, describe the verification gap and the checks
that did run; report the code task as verification-blocked, not complete.
Preserve warning findings and the report's stated limits even after a pass.

## Select a safe workflow

Route the requested capability before editing policy:

| Request | Reference and control surface |
|---|---|
| Adopt an existing repository, inspect historical resources, or trim acquisition inputs | [Existing-repository trimming](references/operations.md#existing-repository-trimming); `init`, reviewed `exclude` globs, then the intended snapshot check |
| Bound instruction size or retain owner/test files, including unchanged files | [File contracts](references/file-contracts.md); project DSL `file` + `change: all`, validated through `rules validate` |
| Require changed independent tests to expose assertion counterexamples on old code | [Test effectiveness](references/test-effectiveness.md); explicit command/task `test_effectiveness` |
| Prevent existing analyzer debt from increasing | [Diagnostic ratchets](references/diagnostic-ratchets.md); command-check `reports[].mode: ratchet`, with a fresh base run |
| Extract enforceable obligations from a policy document | [Rule authoring](references/rule-authoring.md); `rules source`, `schema`, `validate`, `generate` |
| Compose typed entity predicates, counts and bounded collection checks | [Operators](references/operators.md); capability `rules.operators.v2`, `schema operator-rule`, then standard validation/generation |
| Relate named fact collections with lexical lambdas, projections and set checks | [V3 operators](references/operators-v3.md); capability `rules.operators.v3`, `schema operator-rule-v3`, then standard source validation/generation |
| Track every declared clause, its adoption, unsupported gaps and snapshot execution | [Constraint coverage](references/constraint-coverage.md); `schema constraint`, `config --show`, automatic `constraint-coverage` check |
| Apply adopted behavior obligations to the consuming Agent | [Agent contracts](references/agent-contracts.md); `schema agent-obligation`, `agent-contract`; host enforcement needs an actual host |
| Run the selected Rust Agent host with guarded delivery | [Agent host](references/agent-host.md); `agent-run`, current injection receipts, fresh full checks and immutable export |
| Select rules, retrieve policy context, or maintain evidence-backed candidates | [Rule management](references/rule-management.md); `rules context`, candidate validation, signed adoption and history |
| Supply bounded repair context to an external Agent | [Agent feedback](references/agent-feedback.md); `check --feedback`, complete report references and full rechecks |
| Consume versioned decisions or assess warning findings with an external provider | [Decision protocol](references/decision-protocol.md); exported decision/feedback schemas, optional `--envelope`, and shadow/advisory judgment |
| Seal, authorize, independently accept or summarize a predeclared pilot inventory | [Pilot evidence](references/pilot-evidence.md); plan and acceptance subjects, signed external owner/reviewer records, full-report bindings and unknown metrics |
| Verify the installed runtime or repair the CLI | [Selfcheck](references/selfcheck.md); bounded fixture/golden agreement |

File contracts are project rules; diagnostic ratchets configure reports from
existing analyzers. Neither is a new built-in rule ID for `rules enable`.
Use the matching Skill/runtime pair: the version alone cannot distinguish
development builds. Compare exported `rules schema` with the shipped schema
for project authoring, and validate ratchet configuration with the selected
runtime as described in its reference. An unsupported field is a compatibility
gap, not a reason to omit the requested enforcement.

For tool regression, installation verification, or an authorized Qualitygate
implementation repair, use the bundled fixture workflow in
[selfcheck](references/selfcheck.md). It needs no repository policy. A full
corpus agreement is evidence only for its tested shapes and assumptions;
retain golden mismatches and execution gaps separately. After implementation
changes, run the full selfcheck and relevant repository gates before updating
the installed skill/runtime. An explicitly requested local development update
may install that verified build; identify it as a development build rather
than a published release.

Start repository verification with the read-only configuration prerequisite:

```bash
"$QUALITYGATE_BIN" --root "$REPOSITORY_ROOT" --config "$QUALITYGATE_CONFIG" config --show --format json
```

If configuration is missing, automatically run `init` as described above and
verify that the initialization prerequisite is satisfied. Continue with read-only
rule discovery only when the required policy is available:

```bash
"$QUALITYGATE_BIN" --root "$REPOSITORY_ROOT" --config "$QUALITYGATE_CONFIG" rules list --format json
```

Ordinary `init` automatically creates a missing candidate configuration.
`rules enable` updates a candidate configuration and still requires the user's
authorization, as do command adoption with `--with-checks` and policy adoption.
Do not invent a trusted policy reference, task contract, trust store, evidence
directory, severity, or rule selection to make a result pass.

## Select bundled rules deliberately

For progressive discovery and authorized candidate edits, use
[rule management](references/rule-management.md): mutable categories,
category filters, assignment, describe, configure, enable and disable. Start
with `rules categories --format json`, then drill down with `rules list
--category <name>` and `rules describe <id>`. Category changes do not enable
checks or approve policy adoption.

For language-specific discovery use `rules list --language rust --source builtin`
or `--source project` / `--source all`, preferably with `--format json`.
The inventory includes language-neutral rules and unselected built-in packages;
it is not the active enforcement plan. Project discovery defaults to
`qualitygate/rules`, including before `init`. A policy's explicit legacy
`custom_rules` directory is retained for compatibility. Inspect `enabled`,
`source`, `language`, and the complete `definition`; no query changes policy.

Before proposing a built-in rule, read the relevant exact definition under
`references/rules/` and the [bundled rule guide](references/builtin-rules.md).
Those YAML files travel with this skill and are read by the matching CLI at
runtime; they are reviewable selection evidence, not editable runtime
configuration. Verify the executable's active catalog before acting:

```bash
"$QUALITYGATE_BIN" --root "$REPOSITORY_ROOT" --config "$QUALITYGATE_CONFIG" rules list --format json
```

The general `security-sensitive-api` and `todo-marker` rules are warning-level
review signals for changed source lines. Explain their bounded pattern scope;
do not call a match proof of a vulnerability or a defect. `import-boundary` is
an architecture rule for Java, Python, Rust, TypeScript, and Go that is
disabled until a repository explicitly supplies `forbidden_imports`; never
enable it with an invented boundary. Use the language-specific Java project
rules only when their documented project evidence is configured.

For a requested check, state the snapshot selector and profile before running
it. `quick` and `--path` are scoped feedback; they cannot establish delivery
readiness. For a code task without a separate task contract, check the final
worktree against the repository's existing policy. Add the caller's selected
task and policy inputs when that workflow requires them:

```bash
"$QUALITYGATE_BIN" check --root "$REPOSITORY_ROOT" --config "$QUALITYGATE_CONFIG" --worktree --profile quick --format json
"$QUALITYGATE_BIN" check --root "$REPOSITORY_ROOT" --config "$QUALITYGATE_CONFIG" --worktree --profile full --format json
"$QUALITYGATE_BIN" check --root "$REPOSITORY_ROOT" --config "$QUALITYGATE_CONFIG" --worktree --profile full \
  --task tasks/request.yaml --policy-ref "$TRUSTED_COMMIT" \
  --trust-store /trusted/qualitygate/trust.json \
  --evidence-dir /trusted/qualitygate/records --format json
```

`check` can execute the repository commands selected by policy. Keep its scope,
tooling, credentials, timeout behavior, and output location visible to the
user; do not treat an absent tool, missing report, timeout, or partial evidence
as a success. Read [operations](references/operations.md) before handling task,
merge-request, manual-acceptance, or trusted-policy flows.

## Convert repository constraints into structured rules

When asked to enforce `AGENTS.md`, `Agent.md`, contributor guidance, or another
repository policy, first read [rule authoring](references/rule-authoring.md) and
the complete
[project rule schema](references/schemas/project-rule.schema.json). For typed
v2 expressions, also read [operators](references/operators.md), export
`schema operator-rule`, and compare the complete packaged v2 schema. Then:

1. Use `rules list --source all` to avoid duplicating a built-in, project rule,
   lint, or report adapter.
2. Use `rules schema --format json` and compare the parsed result with the
   packaged v1 Schema, or `schema operator-rule` for a v2 candidate. A mismatch
   is a compatibility gap; do not put expressions into v1.
3. Use `rules source --document ... --section ... --format json` to obtain the
   exact source object. Never calculate or guess its digest in prose.
4. Classify every clause. Translate only subjects and assertions supported by
   the finite DSL. Keep unrepresentable clauses visible and route them to the
   evidence owner that can actually prove them.
5. Draft one complete candidate per obligation, then require
   `rules validate candidate.yaml --format json` before any publication.
6. Only after authorized validation, use
   `rules generate --input candidate.yaml --format json`, inspect the generated
   file, validate the whole project-rule directory, and report that adoption is
   still a separate policy/review action.

Do not substitute a prose example for Schema validation. Source binding,
capability declaration, semantic validation, policy selection, source review,
and runtime evidence are separate obligations. Generation creates a candidate
under `qualitygate/rules`; it does not enable the rule or issue approval.

## Interpret results

Exit code `0` for formal `check` means a complete passing gate, `1` means a blocking violation,
and `2` means incomplete validation. Preserve the report and its evidence; do
not convert an incomplete result to pass or claim that a warning filter changes
the computed gate.

Doctor uses exit `0` only for its requested preflight scope and `2` for blocked
or incomplete preflight; it never establishes delivery readiness.

This skill operates through the CLI only. It does not configure MCP, create
manual approvals, alter Git configuration, publish external results, or bypass
the caller's authorization boundary.

When reporting `check` or `selfcheck`, preserve `verification.conclusion`,
`verified_shapes`, `known_limits` and `unverified_assumptions`. A clean result
means “在已验证形态下未发现问题”, not proof that a real deployment is problem-free.
Retain warning findings even if the blocking gate is satisfied.
