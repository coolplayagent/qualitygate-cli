# Resolve the runtime

Read only the example for the active OS and shell. Set the Skill root to the
absolute directory containing `SKILL.md`, `references/` and, in a complete
release archive, `assets/`. Runtime selection needs no repository or policy.

Preserve an explicitly selected `QUALITYGATE_BIN` (Bash) or `$QualitygateBin`
(PowerShell). It must identify the authorized absolute executable path; an
invalid explicit selection is an error, not permission to switch builds.
Otherwise prefer the matching bundled asset, then an already verified
published executable on PATH when that asset is missing or unusable. A user
may explicitly select a verified development build for authorized CLI work;
identify it as a development build rather than a published release.

Establish the selected binary's provenance before running these examples.
PATH lookup, `--version`, a capabilities response and a self-reported digest
do not authenticate a binary. Do not build a repository checkout, install a
runtime or replace an installed copy merely to make startup succeed.

Record the executable and rule-asset bindings and reapply them in each fresh
tool shell. Complete archives discover their own assets. An explicit or PATH
runtime uses this matching Skill's assets when no override was supplied.
Preserve an existing `QUALITYGATE_BUILTIN_RULES_DIR`; it names the directory
containing `core/`, `shared/` and language packages. An invalid explicit override
is an error, not a reason to select different assets. A directory's existence
does not prove its compatibility with the binary.

The examples query `--version` and capabilities only. Compare their results
using [runtime compatibility](runtime-compatibility.md). For an older runtime
without capabilities, follow that reference's history/help path; do not infer
an old version from an arbitrary command failure.

## Linux and WSL: Bash

Set `QUALITYGATE_SKILL_ROOT`. Optionally set `QUALITYGATE_BIN` to the explicitly
selected executable. Linux and WSL use Linux assets; macOS has no bundled asset
and uses an authorized explicit executable or verified PATH runtime.

```bash
: "${QUALITYGATE_SKILL_ROOT:?Set the absolute Skill directory}"
qualitygate_external_runtime=false
if [ "${QUALITYGATE_BIN+x}" = x ]; then
  test -n "$QUALITYGATE_BIN" && test -x "$QUALITYGATE_BIN" || { echo "Invalid explicit Qualitygate executable" >&2; exit 2; }
  "$QUALITYGATE_BIN" --version || exit 2
  qualitygate_external_runtime=true
else
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
    qualitygate_external_runtime=true
  fi
fi
if [ "$qualitygate_external_runtime" = true ] && [ "${QUALITYGATE_BUILTIN_RULES_DIR+x}" != x ]; then
  QUALITYGATE_BUILTIN_RULES_DIR="$QUALITYGATE_SKILL_ROOT/references/rules"
fi
if [ "${QUALITYGATE_BUILTIN_RULES_DIR+x}" = x ]; then
  test -d "$QUALITYGATE_BUILTIN_RULES_DIR" || { echo "Invalid Qualitygate rule asset directory" >&2; exit 2; }
  export QUALITYGATE_BUILTIN_RULES_DIR
fi
"$QUALITYGATE_BIN" capabilities --format json || exit 2
```

## Windows: PowerShell

Set `$QualitygateSkillRoot` to a native absolute path. Optionally set
`$QualitygateBin` to the explicitly selected executable's native absolute path.

```powershell
if (-not $QualitygateSkillRoot) { throw "Set the absolute Skill directory" }
$qualitygateExternalRuntime = $false
if ($null -ne $QualitygateBin) {
  if (-not $QualitygateBin -or -not (Test-Path -LiteralPath $QualitygateBin -PathType Leaf)) {
    throw "Invalid explicit Qualitygate executable"
  }
  & $QualitygateBin --version
  if ($LASTEXITCODE -ne 0) { throw "Explicit Qualitygate runtime is unusable" }
  $qualitygateExternalRuntime = $true
} else {
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
    $qualitygateExternalRuntime = $true
  }
}
if ($qualitygateExternalRuntime -and -not (Test-Path Env:QUALITYGATE_BUILTIN_RULES_DIR)) {
  $env:QUALITYGATE_BUILTIN_RULES_DIR = Join-Path $QualitygateSkillRoot "references/rules"
}
if (Test-Path Env:QUALITYGATE_BUILTIN_RULES_DIR) {
  if (-not (Test-Path -LiteralPath $env:QUALITYGATE_BUILTIN_RULES_DIR -PathType Container)) {
    throw "Invalid Qualitygate rule asset directory"
  }
}
& $QualitygateBin capabilities --format json
if ($LASTEXITCODE -ne 0) { throw "Qualitygate capabilities are unavailable" }
```

## Windows: native Git Bash/MSYS2

Native Windows `.exe` files work in Git Bash/MSYS2. Supply shell paths for the
Skill root, an explicit executable and any asset override. Linux and WSL use
the Linux example. Convert paths passed to native Windows processes explicitly.

```bash
: "${QUALITYGATE_SKILL_ROOT:?Set the absolute Skill directory}"
case "$(uname -s)" in
  MINGW*|MSYS*) ;;
  *) echo "This example requires Windows Git Bash/MSYS2" >&2; exit 2 ;;
esac
qualitygate_external_runtime=false
if [ "${QUALITYGATE_BIN+x}" = x ]; then
  test -n "$QUALITYGATE_BIN" && test -x "$QUALITYGATE_BIN" || { echo "Invalid explicit Qualitygate executable" >&2; exit 2; }
  "$QUALITYGATE_BIN" --version || exit 2
  qualitygate_external_runtime=true
else
  qualitygate_asset=""
  case "$(uname -m)" in
    x86_64|amd64) qualitygate_asset=windows-x86_64 ;;
    aarch64|arm64) qualitygate_asset=windows-aarch64 ;;
  esac
  QUALITYGATE_BIN="$QUALITYGATE_SKILL_ROOT/assets/$qualitygate_asset/qualitygate.exe"
  if [ -z "$qualitygate_asset" ] || ! test -x "$QUALITYGATE_BIN" || ! "$QUALITYGATE_BIN" --version; then
    QUALITYGATE_BIN="$(command -v qualitygate)" || exit 2
    "$QUALITYGATE_BIN" --version || exit 2
    qualitygate_external_runtime=true
  fi
fi
if [ "$qualitygate_external_runtime" = true ] && [ "${QUALITYGATE_BUILTIN_RULES_DIR+x}" != x ]; then
  QUALITYGATE_BUILTIN_RULES_DIR="$QUALITYGATE_SKILL_ROOT/references/rules"
fi
if [ "${QUALITYGATE_BUILTIN_RULES_DIR+x}" = x ]; then
  test -d "$QUALITYGATE_BUILTIN_RULES_DIR" || { echo "Invalid Qualitygate rule asset directory" >&2; exit 2; }
  QUALITYGATE_BUILTIN_RULES_DIR="$(cygpath -m "$QUALITYGATE_BUILTIN_RULES_DIR")" || exit 2
  export QUALITYGATE_BUILTIN_RULES_DIR
fi
MSYS2_ENV_CONV_EXCL=QUALITYGATE_BUILTIN_RULES_DIR MSYS2_ARG_CONV_EXCL='*' \
  "$QUALITYGATE_BIN" capabilities --format json || exit 2
```

Use the same per-command conversion exclusions for later native invocations.
For repository commands convert the root with `cygpath -m`; keep `--config`
repository-relative. The [verification workflow](verification.md#inspect-configuration)
provides configuration-inspection examples for each shell.

## Continue with the requested mode

For installation or evaluator regression, use [selfcheck](selfcheck.md).
Help, capabilities and schema export also require no repository initialization.
Read-only rule discovery may inspect a repository without creating policy;
follow [rule management](rule-management.md).

For a repository check, follow [verification](verification.md) to establish the
selected policy and inspect preflight evidence. Runtime resolution alone does
not require `config --show`, `init`, doctor or a formal check.
