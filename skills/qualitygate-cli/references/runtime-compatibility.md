# Runtime compatibility and doctor evidence

Use [runtime startup](runtime-startup.md) to select the executable. Compare
`--version` with Skill metadata, inspect
`--help`, then query `capabilities --format json` when advertised. Validate with
[capabilities schema](schemas/capabilities.schema.json). Record the executable
SHA-256, not just the version: builds with equal versions may differ. If the
capability command is absent, use the history table and `check --help`; do not
invent a capabilities response or relabel an arbitrary parse error as age.
Identity and compatibility evidence do not authenticate the binary's provenance.
Require capabilities for the requested operation; reading this reference does
not turn a runtime query or selfcheck into repository verification.

| Capability | First published version | Verified history |
| --- | --- | --- |
| Declared bounded tool version probes | v0.3.0 | `a7f9550` belongs to v0.3.0 |
| File contracts and report ratchets | v0.4.0 | `baa1eb3` belongs to v0.4.0 |
| `exclude`, `--snapshot-max-file-mib`, `init.snapshot_preflight` | v0.5.5 | `98c0a66`; absent in v0.5.1–v0.5.4 tagged source |
| Capabilities, runtime requirements, environment requirements and doctor | v0.5.7 | `82f51a8`; require exported capability IDs; v0.5.6 lacks these additions |
| Constraint coverage and typed bounded operators | v0.5.8 | `aaa82ff`, `59c91f5`; require `constraints.coverage.v1` and `rules.operators.v2` |
| Consuming-agent contracts, Rust host and immutable delivery | v0.5.8 | `a780603`, `4579f40`, `8c4488a`; require `agent.contracts.v1`, `agent.host.v1` and `agent.delivery.v1` |
| Default document/media exclusions, retention and classified inventory evidence | v0.5.9 | `be889e9`; require `snapshot.unstructured-documents.v1` |
| Named collections and lexical lambdas | v1.0.0 | `b8cb5c8`; require `rules.operators.v3` and export `schema operator-rule-v3`; v1/v2 remain supported. |

From v0.5.9, default document/media exclusions also apply to existing policies.
Compare `snapshot.unstructured-documents.v1` when relying on this behavior.
Restore command dependencies with `unstructured_documents.retain`, or set
`enabled: false` to disable defaults. See [operations](operations.md#default-document-and-media-acquisition-exclusions).

Repository preflight uses v0.5.7 or a later compatible runtime. Require
`runtime.capabilities.v1` and `doctor.static.v1`; require `config.requires.v1`,
`command.required-env.v1` and `doctor.tool-probes.v1` when using those contracts.
Old runtime conclusions remain valid only for the inspected build. Select an
already verified compatible install or report the capability gap. Existing
authorization may cover selecting or updating a development build; use the
[implementation workflow](selfcheck.md) for its verification. Do not delete
unsupported fields or lower requirements to accommodate an incompatible binary.

The [verification workflow](verification.md) owns configuration prerequisites,
doctor invocation and final-check acceptance. Preserve its selected root/config,
snapshot, profile, task, policy and acquisition budgets. Doctor currently has no
MR selector; retain that preflight gap without changing the requested comparison.

Default doctor only inspects prerequisites. `--probe-tools` executes selected
`tools[].argv` in private snapshots, in sequence, with a shared 120-second budget
and each probe's timeout. It never executes `checks[].argv`. Select probes when
the task authorizes producer execution; a normal requested repository check
already includes these declared probes. Do not claim installation/dependency
readiness from executable presence alone. Doctor does not infer environment
names embedded in scripts; declare `required_env` in command checks or task
verification. Names must exist and have nonempty values; never print values.
Formal check enforces the declarations too.

For a runtime advertising `doctor.summary.v1`, `--summary --format json` avoids
terminal truncation. Apply it to the requested doctor invocation.

Default table/Markdown output also uses this summary. The
[summary schema](schemas/doctor-summary.schema.json) is separate from the full
doctor schema. `report.path` names a retained `qualitygate-doctor-*/report.json`
in OS temporary storage; `report.digest` and `report.bytes` identify its contents.
Read the saved report to inspect all diagnostics, identities, budgets, unexecuted
items and `next_steps[].command`. Summary details are limited to five diagnostics
and five next steps, with 240 characters of each text field; counts are complete
and `details_truncated` signals omitted summary detail. The saved report retains
the existing full protocol, including its own inventory detail limits.
Exit 2 still means blocked/incomplete. A failed write or the 16 MiB report limit
returns `report.write_failed` and `report: null`, never a successful receipt.
Delete the private temporary directory when finished; it survives CLI exit and
is not governed by `QUALITYGATE_HOME`. No repository output files are created.
Older runtimes retain `--format json`; redirect stdout to a temporary file and
capture the CLI exit status before parsing it. Do not pass an unsupported flag.

Validate the report with [doctor schema](schemas/doctor.schema.json). Retain
`scope: preflight`, identities, requested probe scope, diagnostics, unexecuted
items, pending checks and `next_steps`. Treat `next_steps[].command` as argv and
check it against the requested scope and existing authorization before execution.
Exit 0 means the requested preflight passed, not delivery acceptance. Exit 2 is
blocked/incomplete. Doctor's temporary probe evidence is not a check credential.
For a repository verification task, return to the verification workflow for the
formal check. A doctor-only request ends with its preflight evidence and limits.

`runtime.version_too_old` and `runtime.capability_missing` require a compatible
build; `config.unknown_field`, `config.type` and `config.syntax` require inspection
of the original error and available field/line/column. These are distinct from
an absent selected policy; use the verification workflow to handle that case.

## Acquisition diagnostics

Inspect selected-snapshot budgets. `init.snapshot_preflight` is the advisory
HEAD/worktree discovery report; doctor distinguishes base/index/diff inventories,
full counts, total bytes, unsupported entries and protected exclusions.

Default capacity remains 2 MiB; the maximum is 8 MiB. Prefer the smallest
sufficient integer MiB suggestion. Raising total `--snapshot-max-mib` does not
raise the per-file limit. Use metadata or repository instructions to justify a
capacity change. Above 8 MiB, review explicit policy exclusions for
unrelated resources; never automatically exclude or alter verification inputs.
Excluded files are unavailable to commands. Base-only oversize still matters,
and truncated detail lists retain complete counts. Follow
[snapshot and policy selection](operations.md#snapshot-and-policy-selection)
and [repository verification](verification.md) when applying a reviewed change.
