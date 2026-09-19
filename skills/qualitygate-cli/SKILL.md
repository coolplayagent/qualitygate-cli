---
name: qualitygate-cli
description: "Verify repository code changes with Qualitygate CLI before declaring implementation, bug-fix, or refactor tasks complete. Run a full snapshot-bound check against existing policy and any task contract, repair violations or incomplete evidence, and recheck the final snapshot. Also turn repository instructions such as AGENTS.md into source-bound, schema-validated project-rule candidates through the matching CLI. Use for file contracts, diagnostic ratchets, rule configuration, and selfcheck; not generic review advice."
metadata:
  version: "0.5.2"
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

## Resolve the executable

Release archives place a platform binary under this skill's `assets/` directory.
Use its absolute path only when it matches the active operating system and
`--version` succeeds. A lightweight registry installation may omit assets; in
that case use a verified published `qualitygate` executable on `PATH`. Do not
silently build an untrusted repository checkout as the skill runtime.

On a POSIX shell, set `QUALITYGATE_SKILL_ROOT` to the directory containing this
`SKILL.md`, point the CLI at this Skill's external rule assets, and select the
matching Linux asset before falling back to `PATH`:

```bash
QUALITYGATE_BUILTIN_RULES_DIR="$QUALITYGATE_SKILL_ROOT/references/rules"
export QUALITYGATE_BUILTIN_RULES_DIR
if ! test -d "$QUALITYGATE_BUILTIN_RULES_DIR"; then
  echo "Qualitygate Skill rule assets are missing" >&2
  exit 2
fi
case "$(uname -m)" in
  x86_64|amd64) QUALITYGATE_ASSET="assets/linux-x86_64/qualitygate" ;;
  aarch64|arm64) QUALITYGATE_ASSET="assets/linux-aarch64/qualitygate" ;;
  *) QUALITYGATE_ASSET="" ;;
esac
QUALITYGATE_BIN="$QUALITYGATE_SKILL_ROOT/$QUALITYGATE_ASSET"
if [ -z "$QUALITYGATE_ASSET" ] || ! test -x "$QUALITYGATE_BIN" || ! "$QUALITYGATE_BIN" --version; then
  QUALITYGATE_BIN="$(command -v qualitygate)"
  "$QUALITYGATE_BIN" --version
fi
```

On PowerShell, use the matching Windows asset only from a Windows shell:

```powershell
$architecture = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
$env:QUALITYGATE_BUILTIN_RULES_DIR = Join-Path $QualitygateSkillRoot "references/rules"
if (-not (Test-Path -LiteralPath $env:QUALITYGATE_BUILTIN_RULES_DIR -PathType Container)) {
  throw "Qualitygate Skill rule assets are missing"
}
$assetDirectory = if ($architecture -eq "X64") {
  "windows-x86_64"
} elseif ($architecture -eq "Arm64") {
  "windows-aarch64"
} else {
  $null
}
$QualitygateBin = if ($assetDirectory) {
  Join-Path $QualitygateSkillRoot "assets/$assetDirectory/qualitygate.exe"
} else {
  $null
}
$qualitygateReady = $false
if ($QualitygateBin -and (Test-Path -LiteralPath $QualitygateBin)) {
  & $QualitygateBin --version
  $qualitygateReady = $LASTEXITCODE -eq 0
}
if (-not $qualitygateReady) {
  $QualitygateBin = (Get-Command qualitygate -ErrorAction Stop).Source
  & $QualitygateBin --version
}
```

Do not run `qualitygate.exe` from bash, sh, zsh, fish, or WSL bash. If neither
asset nor a published `PATH` executable works, report that installation is
needed rather than changing the repository or global tool configuration.

## Verify code tasks before completion

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
the final report has `profile: full`, `scope: repository` or `scope: task`
(never `scope: path`), an empty `plan.pending_delivery_checks`,
`gate.complete: true`, and `gate.decision: pass`, with exit code `0`, and all
separately required repository checks pass. Retain its snapshot and policy
digests, and rerun if the checked inputs change.

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
| Bound instruction size or retain owner/test files, including unchanged files | [File contracts](references/file-contracts.md); project DSL `file` + `change: all`, validated through `rules validate` |
| Require changed independent tests to expose assertion counterexamples on old code | [Test effectiveness](references/test-effectiveness.md); explicit command/task `test_effectiveness` |
| Prevent existing analyzer debt from increasing | [Diagnostic ratchets](references/diagnostic-ratchets.md); command-check `reports[].mode: ratchet`, with a fresh base run |
| Extract enforceable obligations from a policy document | [Rule authoring](references/rule-authoring.md); `rules source`, `schema`, `validate`, `generate` |
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

Start with read-only discovery when the user has not asked to change policy:

```bash
"$QUALITYGATE_BIN" --root "$REPOSITORY_ROOT" rules list --format json
"$QUALITYGATE_BIN" --root "$REPOSITORY_ROOT" config --show --format json
```

`init` creates a candidate configuration and `rules enable` updates a candidate
configuration. Run either only after the user asks for that mutation. Do not
invent a trusted policy reference, task contract, trust store, evidence
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
"$QUALITYGATE_BIN" --root "$REPOSITORY_ROOT" rules list --format json
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
"$QUALITYGATE_BIN" check --root "$REPOSITORY_ROOT" --worktree --profile quick --format json
"$QUALITYGATE_BIN" check --root "$REPOSITORY_ROOT" --worktree --profile full --format json
"$QUALITYGATE_BIN" check --root "$REPOSITORY_ROOT" --worktree --profile full \
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
[project rule schema](references/schemas/project-rule.schema.json). Then:

1. Use `rules list --source all` to avoid duplicating a built-in, project rule,
   lint, or report adapter.
2. Use `rules schema --format json` and compare the parsed result with the
   packaged Schema. A mismatch is a compatibility gap.
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

Exit code `0` means a complete passing gate, `1` means a blocking violation,
and `2` means incomplete validation. Preserve the report and its evidence; do
not convert an incomplete result to pass or claim that a warning filter changes
the computed gate.

This skill operates through the CLI only. It does not configure MCP, create
manual approvals, alter Git configuration, publish external results, or bypass
the caller's authorization boundary.

When reporting `check` or `selfcheck`, preserve `verification.conclusion`,
`verified_shapes`, `known_limits` and `unverified_assumptions`. A clean result
means “在已验证形态下未发现问题”, not proof that a real deployment is problem-free.
Retain warning findings even if the blocking gate is satisfied.
