# Runtime compatibility and preflight

Resolve the executable first. Compare `--version` with Skill metadata, inspect
`--help`, then query `capabilities --format json` when advertised. Validate with
[capabilities schema](schemas/capabilities.schema.json). Record the executable
SHA-256, not just the version: builds with equal versions may differ. If the
capability command is absent, use the history table and `check --help`; do not
invent a capabilities response or relabel an arbitrary parse error as age.

| Capability | First published version | Verified history |
| --- | --- | --- |
| Declared bounded tool version probes | v0.3.0 | `a7f9550` belongs to v0.3.0 |
| File contracts and report ratchets | v0.4.0 | `baa1eb3` belongs to v0.4.0 |
| `exclude`, `--snapshot-max-file-mib`, `init.snapshot_preflight` | v0.5.5 | `98c0a66`; absent in v0.5.1–v0.5.4 tagged source |
| Capabilities, runtime requirements, environment requirements and doctor | Unreleased after v0.5.6 | Require exported capability IDs; released v0.5.6 lacks these additions |

This development Skill retains v0.5.6 metadata until release preparation. A
v0.5.6 version match alone is insufficient. Require `runtime.capabilities.v1`,
`config.requires.v1`, `command.required-env.v1`, `doctor.static.v1`, and
`doctor.tool-probes.v1` for this workflow. Old runtime conclusions remain valid
only for the inspected build. Select an already verified compatible install or
report the capability gap; do not install tools, build untrusted checkouts,
delete unsupported fields, or lower requirements automatically.

After checking configuration existence/compatibility, run doctor with exactly
the intended selector, profile, task, policy reference and acquisition budgets:

```bash
"$QUALITYGATE_BIN" doctor --root "$REPOSITORY_ROOT" \
  --diff origin/1.8.x-alpha..HEAD --profile full --format json
"$QUALITYGATE_BIN" doctor --root "$REPOSITORY_ROOT" \
  --diff origin/1.8.x-alpha..HEAD --profile full --probe-tools --format json
```

Default doctor only inspects prerequisites. `--probe-tools` executes selected
`tools[].argv` in private snapshots, in sequence, with a shared 120-second budget
and each probe's timeout. It never executes `checks[].argv`. Select probes when
the task authorizes producer execution; a normal requested repository check
already includes these declared probes. Do not claim installation/dependency
readiness from executable presence alone. Doctor does not infer environment
names embedded in scripts; declare `required_env` in command checks or task
verification. Names must exist and have nonempty values; never print values.
Formal check enforces the declarations too.

Validate the report with [doctor schema](schemas/doctor.schema.json). Retain
`scope: preflight`, identities, requested probe scope, diagnostics, unexecuted
items, pending checks and `next_steps`. Invoke `next_steps[].command` as argv.
Exit 0 means the requested preflight passed, not delivery acceptance. Exit 2 is
blocked/incomplete. Doctor's temporary probe evidence is not a check credential.
Always finish with the formal unfiltered full check on the final snapshot.

`runtime.version_too_old` and `runtime.capability_missing` require a compatible
build; `config.unknown_field`, `config.type` and `config.syntax` require inspection
of the original error and available field/line/column. Do not repeatedly init an
existing valid configuration or overwrite invalid YAML. Follow the existing
initialization prerequisite only when the required configuration is absent.

## Large repository template

First inspect selected-snapshot budgets. A valid repository configuration does
not require another init; `init.snapshot_preflight` is only the advisory
HEAD/worktree discovery report. Doctor distinguishes base/index/diff inventories,
full counts, total bytes, unsupported entries and protected exclusions.

For repositories whose metadata or explicit instructions justify 8 MiB:

```bash
"$QUALITYGATE_BIN" doctor --root "$REPOSITORY_ROOT" --profile full \
  --diff origin/1.8.x-alpha..HEAD --snapshot-max-file-mib 8 --format json
"$QUALITYGATE_BIN" check --root "$REPOSITORY_ROOT" --profile full \
  --diff origin/1.8.x-alpha..HEAD --snapshot-max-file-mib 8 --format json
```

Default capacity remains 2 MiB; the maximum is 8 MiB. Prefer the smallest
sufficient integer MiB suggestion. Raising total `--snapshot-max-mib` does not
raise the per-file limit. Above 8 MiB, review explicit policy exclusions for
unrelated resources; never automatically exclude or alter verification inputs.
Excluded files are unavailable to commands. Base-only oversize still matters,
and truncated detail lists retain complete counts. Stage/commit a reviewed
policy for index/diff selection; never switch to worktree to evade that boundary.
