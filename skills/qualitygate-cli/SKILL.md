---
name: qualitygate-cli
description: "Operate Qualitygate CLI for snapshot-bound repository policy and task acceptance checks; use when a user asks to inspect, initialize, configure, or run quality gates, not for generic code-review advice or bypassing evidence requirements."
metadata:
  version: "0.2.0"
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

## Interpret results

Exit code `0` means a complete passing gate, `1` means a blocking violation,
and `2` means incomplete validation. Preserve the report and its evidence; do
not convert an incomplete result to pass or claim that a warning filter changes
the computed gate.

This skill operates through the CLI only. It does not configure MCP, create
manual approvals, alter Git configuration, publish external results, or bypass
the caller's authorization boundary.
