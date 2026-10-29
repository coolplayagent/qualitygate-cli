# Resolve the runtime and inspect configuration

Read only the example for the active OS and shell. Set the Skill and repository
roots to absolute paths; the Skill root contains `SKILL.md`, `references/` and,
in a complete release archive, `assets/`. Set the configuration to the caller's
repository-relative path, or keep `qualitygate.yaml`. Spaces are supported.

Record the resolved executable and root/config/asset bindings. Reapply them in
each fresh tool shell; neither exports nor shell variables necessarily persist.
An existing `QUALITYGATE_BUILTIN_RULES_DIR` takes precedence over automatic asset
discovery. Preserve it, inspect the selected directory and runtime compatibility,
and report an invalid override instead of silently replacing it. The variable
points to the directory containing `core/`, `shared/` and the language packages.

Complete archives need no override. A verified published PATH runtime uses this
Skill's assets when no override was supplied. Version/capability checks below
establish runtime identity; compare them with the
[compatibility workflow](runtime-compatibility.md) before proceeding. A binary
found on PATH is not proven published or compatible merely because it runs.

## Linux and WSL: Bash

Set `QUALITYGATE_SKILL_ROOT` and `REPOSITORY_ROOT` first. This example selects a
bundled Linux binary only on Linux; another OS can use a verified PATH runtime.
In particular, macOS has no bundled asset and must not select a Linux binary.

```bash
: "${QUALITYGATE_SKILL_ROOT:?Set the absolute Skill directory}"
: "${REPOSITORY_ROOT:?Set the absolute repository directory}"
QUALITYGATE_CONFIG="${QUALITYGATE_CONFIG-qualitygate.yaml}"
qualitygate_asset=""
if [ "$(uname -s)" = Linux ]; then
  case "$(uname -m)" in
    x86_64|amd64) qualitygate_asset=linux-x86_64 ;;
    aarch64|arm64) qualitygate_asset=linux-aarch64 ;;
  esac
fi
QUALITYGATE_BIN="$QUALITYGATE_SKILL_ROOT/assets/$qualitygate_asset/qualitygate"
if [ -z "$qualitygate_asset" ] || ! test -x "$QUALITYGATE_BIN" || ! "$QUALITYGATE_BIN" --version; then
  QUALITYGATE_BIN="$(command -v qualitygate)" || exit 2
  "$QUALITYGATE_BIN" --version || exit 2
  if [ "${QUALITYGATE_BUILTIN_RULES_DIR+x}" != x ]; then
    QUALITYGATE_BUILTIN_RULES_DIR="$QUALITYGATE_SKILL_ROOT/references/rules"
  fi
fi
if [ "${QUALITYGATE_BUILTIN_RULES_DIR+x}" = x ]; then
  test -d "$QUALITYGATE_BUILTIN_RULES_DIR" || { echo "Invalid Qualitygate rule asset directory" >&2; exit 2; }
  export QUALITYGATE_BUILTIN_RULES_DIR
fi
"$QUALITYGATE_BIN" capabilities --format json || exit 2
"$QUALITYGATE_BIN" config --show --root "$REPOSITORY_ROOT" --config "$QUALITYGATE_CONFIG" --format json
```

## Windows: PowerShell

Set `$QualitygateSkillRoot` and `$RepositoryRoot` to native absolute paths.
Set `$QualitygateConfig` when the caller selected a nondefault configuration.

```powershell
if (-not $QualitygateSkillRoot -or -not $RepositoryRoot) {
  throw "Set the absolute Skill and repository directories"
}
if ($null -eq $QualitygateConfig) { $QualitygateConfig = "qualitygate.yaml" }
$architecture = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
$assetDirectory = if ([System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform([System.Runtime.InteropServices.OSPlatform]::Windows)) {
  if ($architecture -eq "X64") { "windows-x86_64" }
  elseif ($architecture -eq "Arm64") { "windows-aarch64" }
} else { $null }
$QualitygateBin = if ($assetDirectory) {
  Join-Path $QualitygateSkillRoot "assets/$assetDirectory/qualitygate.exe"
} else { $null }
$qualitygateReady = $false
if ($QualitygateBin -and (Test-Path -LiteralPath $QualitygateBin -PathType Leaf)) {
  try {
    & $QualitygateBin --version
    $qualitygateReady = $LASTEXITCODE -eq 0
  } catch { $qualitygateReady = $false }
}
if (-not $qualitygateReady) {
  $QualitygateBin = (Get-Command qualitygate -CommandType Application -ErrorAction Stop | Select-Object -First 1).Source
  & $QualitygateBin --version
  if ($LASTEXITCODE -ne 0) { throw "Qualitygate runtime is unusable" }
  if (-not (Test-Path Env:QUALITYGATE_BUILTIN_RULES_DIR)) {
    $env:QUALITYGATE_BUILTIN_RULES_DIR = Join-Path $QualitygateSkillRoot "references/rules"
  }
}
if (Test-Path Env:QUALITYGATE_BUILTIN_RULES_DIR) {
  if (-not (Test-Path -LiteralPath $env:QUALITYGATE_BUILTIN_RULES_DIR -PathType Container)) {
    throw "Invalid Qualitygate rule asset directory"
  }
}
& $QualitygateBin capabilities --format json
if ($LASTEXITCODE -ne 0) { throw "Qualitygate capabilities are unavailable" }
& $QualitygateBin config --show --root $RepositoryRoot --config $QualitygateConfig --format json
# Inspect $LASTEXITCODE and the configuration result before another workflow step.
```

## Windows: native Git Bash/MSYS2

Native Windows `.exe` files work in Git Bash/MSYS2. Use shell paths for the
two roots and for an optional rule-asset override, then explicitly convert the
arguments passed to the Windows executable. Linux and WSL use the Linux example.

```bash
: "${QUALITYGATE_SKILL_ROOT:?Set the absolute Skill directory}"
: "${REPOSITORY_ROOT:?Set the absolute repository directory}"
QUALITYGATE_CONFIG="${QUALITYGATE_CONFIG-qualitygate.yaml}"
case "$(uname -s)" in
  MINGW*|MSYS*) ;;
  *) echo "This example requires Windows Git Bash/MSYS2" >&2; exit 2 ;;
esac
qualitygate_asset=""
case "$(uname -m)" in
  x86_64|amd64) qualitygate_asset=windows-x86_64 ;;
  aarch64|arm64) qualitygate_asset=windows-aarch64 ;;
esac
QUALITYGATE_BIN="$QUALITYGATE_SKILL_ROOT/assets/$qualitygate_asset/qualitygate.exe"
if [ -z "$qualitygate_asset" ] || ! test -x "$QUALITYGATE_BIN" || ! "$QUALITYGATE_BIN" --version; then
  QUALITYGATE_BIN="$(command -v qualitygate)" || exit 2
  "$QUALITYGATE_BIN" --version || exit 2
  if [ "${QUALITYGATE_BUILTIN_RULES_DIR+x}" != x ]; then
    QUALITYGATE_BUILTIN_RULES_DIR="$QUALITYGATE_SKILL_ROOT/references/rules"
  fi
fi
if [ "${QUALITYGATE_BUILTIN_RULES_DIR+x}" = x ]; then
  test -d "$QUALITYGATE_BUILTIN_RULES_DIR" || { echo "Invalid Qualitygate rule asset directory" >&2; exit 2; }
  QUALITYGATE_BUILTIN_RULES_DIR="$(cygpath -m "$QUALITYGATE_BUILTIN_RULES_DIR")" || exit 2
  export QUALITYGATE_BUILTIN_RULES_DIR
fi
"$QUALITYGATE_BIN" capabilities --format json || exit 2
MSYS2_ENV_CONV_EXCL=QUALITYGATE_BUILTIN_RULES_DIR MSYS2_ARG_CONV_EXCL='*' \
  "$QUALITYGATE_BIN" config --show --root "$(cygpath -m "$REPOSITORY_ROOT")" \
  --config "$QUALITYGATE_CONFIG" --format json
```

Use the same per-command MSYS2 conversion exclusions and native root for later
`init`, `doctor` and `check` invocations. Keep `--config` repository-relative.

## Continue from the configuration result

These startup examples do not run `init`, doctor or checks. A missing
configuration is distinct from a missing runtime, missing rule assets, invalid
YAML or an unsupported capability. Follow the
[initialization prerequisite](../SKILL.md#initialization-prerequisite): confirm
absence, run ordinary `init` with the same root/config, then query configuration
again. Do not infer absence from an arbitrary nonzero exit code.

For example, after confirming the selected configuration is absent on Linux:

```bash
"$QUALITYGATE_BIN" init --root "$REPOSITORY_ROOT" --config "$QUALITYGATE_CONFIG" --format json
```

Inspect success and recheck configuration in a separate invocation. Only then
run doctor with the intended snapshot/profile/task/policy/budgets and the formal
check. Do not copy a fixed worktree selector over a requested staged/diff/MR
scope. Runtime-only operations, including selfcheck and schema export, do not
require this repository initialization workflow.
