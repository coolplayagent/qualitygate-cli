# Runtime capabilities and environment preflight

[简体中文](../../zh/02-reference/06-runtime-and-preflight.md) · [Volume index](README.md)

These capabilities are available from v0.5.7. The package version alone
does not establish support: query the capability IDs and executable digest.

```bash
qualitygate capabilities --format json
qualitygate schema capabilities --format json
qualitygate schema doctor --format json
qualitygate doctor --diff origin/1.8.x-alpha..HEAD --profile full --format json
qualitygate doctor --diff origin/1.8.x-alpha..HEAD --profile full --probe-tools --format json
```

`capabilities` works offline without a repository, configuration or rule assets.
It reports `cli_version`, SHA-256 executable identity, OS/architecture, versioned
capability IDs, protocol schema versions and resource limits. Two builds sharing
a version can have different executable digests. The digest identifies bytes;
it does not authenticate a release or replace review of a development build.

`doctor` defaults to worktree/full. It accepts `--worktree`, `--staged`, `--diff
BASE..HEAD`, worktree `--base`, `--profile`, `--task`, `--policy-ref`, and the four
snapshot budget options shared with `check`. It does not accept MR, path-filter,
feedback, or decision-envelope options. Remote MR preflight is outside this
version's scope; use the formal MR workflow with explicit remaining gaps.

## Policy requirements

```yaml
schema_version: 1
requires:
  min_cli_version: '0.5.7'
  capabilities: [config.requires.v1, command.required-env.v1, doctor.static.v1]
checks:
  - id: verify
    argv: [git, --version]
    required_env: [PROJECT_TOOLCHAIN]
    tools:
      - id: git
        argv: [git, --version]
```

`requires` is optional. `min_cli_version` is a semantic version, including
prerelease ordering; `capabilities` requires exact IDs returned by the runtime.
The example declares both version and capability requirements so callers can
identify builds that support this workflow. Empty new fields are omitted
from serialized legacy configurations. Requirements are also validated by the
reusable planner and active-policy paths.

Command checks and task `acceptance[].verification` can declare `required_env`.
Names must be distinct portable identifiers (`[A-Za-z_][A-Za-z0-9_]*`), up to 128
names of 128 bytes each. Only presence and a nonempty value are checked. Values
are neither printed nor inferred from scripts; whitespace and non-UTF-8 values
are nonempty. Formal `check` enforces the same requirement, including paired
command execution. Manual checks cannot declare command environment inputs.

Parsing first validates YAML structure and duplicate keys, then explicit
runtime requirements, then strict configuration types/fields and semantics.
`runtime.version_too_old`, `runtime.capability_missing`, `config.unknown_field`,
`config.type`, and `config.syntax` remain distinct. Diagnostics retain original
parser text and available field/line/column. A spelling error is not automatically
classified as an old runtime. Old executables cannot emit the new diagnostics;
the Skill uses the verified history table, `--version` and command help.

## Execution and claim boundary

1. Resolve the same active policy or caller-selected policy reference, catalog,
   task and profile as `check`; expand selected check dependencies.
2. Inspect metadata for actual base and target (and a separate policy reference),
   applying exclusions and protecting verification inputs. Staged inspection
   reads index blob sizes; diff inspection reads commit endpoints.
3. If budgets fit, capture and materialize immutable inputs. Validate the full
   selected policy, working directories, executable identities, tool input files,
   required arguments and environment names. No project command runs by default.
4. With `--probe-tools`, run only selected `tools[].argv` in order. Reuse the
   formal check's bounded capture, executable identity and input guards. Probes
   share 120 seconds; individual probes keep their configured 1–60 second timeout.
   Source/policy identities are checked again at completion.

Timeouts, output overflow, empty/invalid version text, unsuccessful exits,
changed inputs and changed executables remain incomplete evidence. The report
retains exit/timing, tool-input and output digests, and byte counts. Raw probe
stdout/stderr and version text are not published, because producers can print
inherited credentials. Temporary probe logs disappear with the private workspace.
Checks, rule evaluations and unattempted probes appear in `not_executed`.

The report has `scope: preflight`, runtime/snapshot/policy identities, selected
and pending checks, per-side budgets, diagnostics and `next_steps`. Each optional
`next_steps[].command` is an argv array to invoke directly, never a shell string.
Exit `0` means only the requested preflight passed. Exit `2` means blocked or
incomplete preflight. There is no delivery gate or reusable check/cache credential.
After successful preflight, run the formal unfiltered full `check`.

## Large repository template

Default per-file capacity remains 2 MiB, with an 8 MiB maximum. Metadata reports
list up to 100 details per category while retaining complete counts and an
explicit truncation flag. Base-only oversized files still block acquisition.
For permitted files, the suggestion is the smallest sufficient integer MiB;
above 8 MiB the report states the capacity boundary and suggests reviewing
exclusions. It never edits policy. Increasing `--snapshot-max-mib` cannot fix a
per-file overflow. File-count limits include the inventoried excluded entries.

Use the following only when metadata or repository instructions justify 8 MiB:

```bash
qualitygate doctor --root . --profile full \
  --diff origin/1.8.x-alpha..HEAD --snapshot-max-file-mib 8 --format json
qualitygate check --root . --profile full \
  --diff origin/1.8.x-alpha..HEAD --snapshot-max-file-mib 8 --format json
```

Review selected-policy exclusions for unrelated resources; excluded files are
unavailable to builds. Protected verification and declared tool inputs must stay
available. `init.snapshot_preflight` remains an advisory HEAD/worktree aid;
`doctor` supplies selected-snapshot readiness. A valid existing policy does not
need another `init`.

## Compatibility and shells

Git history and tagged source confirm these earliest published versions:

| Surface | First published version | Evidence |
| --- | --- | --- |
| Declared bounded tool version probes | v0.3.0 | `a7f9550`, contained in v0.3.0 |
| File contracts and report ratchets | v0.4.0 | `baa1eb3`, contained in v0.4.0 |
| `exclude`, per-file budget and `init.snapshot_preflight` | v0.5.5 | `98c0a66`; absent in v0.5.1–v0.5.4 tagged source |
| Runtime capabilities, `requires`, `required_env`, doctor | v0.5.7 | `82f51a8`; require the exported versioned IDs; v0.5.6 lacks these additions |

Use Windows assets in PowerShell or native Windows Git Bash/MSYS2; use Linux
assets in Linux and WSL. Windows Git Bash can invoke `.exe` directly. Quote
paths with spaces and convert repository/asset environment paths with `cygpath
-m`; disable automatic argument conversion only for that explicit native call.
The packaged [Skill workflow](../../../skills/qualitygate-cli/SKILL.md) contains
complete examples. This follows the documented
[MSYS2 path conversion behavior](https://www.msys2.org/docs/filesystem-paths/).

The Windows CI lane executes the Rust regression suite and direct PowerShell
and Git Bash invocations in paths containing spaces. A Linux test run does not
establish native Windows acceptance; retain the actual Windows job result.
