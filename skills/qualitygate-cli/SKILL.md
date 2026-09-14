---
name: qualitygate-cli
description: "Operate Qualitygate CLI for snapshot-bound checks, schema-validated project rules, instruction/file budgets, required-file contracts, diagnostic debt ratchets, and evidence-backed policy evolution. Use for configuring or running these rule-based gates and bundled selfcheck; not generic review advice or evidence bypasses."
metadata:
  version: "0.4.0"
  homepage: "https://github.com/coolplayagent/qualitygate-cli"
---

# Qualitygate CLI

Use the published `qualitygate` executable as the control surface. This skill
does not replace repository policy, task acceptance, review, or signed evidence
with an agent judgment.

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

## Select a safe workflow

Route the requested capability before editing policy:

| Request | Reference and control surface |
|---|---|
| Bound instruction size or retain owner/test files, including unchanged files | [File contracts](references/file-contracts.md); project DSL `file` + `change: all`, validated through `rules validate` |
| Prevent existing analyzer debt from increasing | [Diagnostic ratchets](references/diagnostic-ratchets.md); command-check `reports[].mode: ratchet`, with a fresh base run |
| Extract enforceable obligations from a policy document | [Rule authoring](references/rule-authoring.md); `rules source`, `schema`, `validate`, `generate` |
| Select rules, retrieve policy context, or maintain evidence-backed candidates | [Rule management](references/rule-management.md); `rules context`, candidate validation, signed adoption and history |
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
readiness. A full task check needs the caller's selected task and policy:

```bash
"$QUALITYGATE_BIN" check --root "$REPOSITORY_ROOT" --worktree --profile quick --format json
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

## Extract project rules with the schema

When asked to extract AGENTS.md, Agent.md, or another project policy into
rules, first read [rule authoring](references/rule-authoring.md) and the complete
[project rule schema](references/schemas/project-rule.schema.json). Use the
schema to construct each candidate, then the matching CLI to validate and
publish it under `qualitygate/rules`. Do not substitute a prose example for
schema validation. Source hashes, capabilities, and source review remain
separate obligations; generation does not enable rules or issue approvals.

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
