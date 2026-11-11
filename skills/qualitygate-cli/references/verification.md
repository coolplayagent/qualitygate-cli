# Repository verification

Use this workflow for implementation, bug-fix and refactor completion, or an
explicit repository check. First resolve a trusted matching runtime through
[startup](runtime-startup.md) and [compatibility](runtime-compatibility.md).
Runtime inspection, selfcheck and read-only rule discovery do not require this
workflow. Keep configuration prerequisites, preflight and gate results distinct.

## Initialization prerequisite

Establish whether the required policy exists for the repository and selected
snapshot. Use file/Git inspection or a read-only `config --show` for this
decision. Run this prerequisite separately; do not batch it with later checks.
First identify the policy source rather than assuming the worktree owns it:

| Selected authority | Inspection and missing-policy response |
| --- | --- |
| Active signed policy | `config --show` returns the authenticated active package with `review_trust: signed_active_policy` and its `source`, even if the local candidate or caller's `--config` path is absent. Authentication failures require diagnosis; local `init` cannot repair them. |
| Explicit `--policy-ref`, without an active policy | Inspect that reference and its selected configuration, for example with `rules context --policy-ref <selected-ref>`. If missing or invalid, report the reference prerequisite; do not initialize an unrelated worktree candidate. |
| Selected snapshot, without either authority above | Inspect the worktree candidate or selected index/commit as appropriate. `config --show` reports `review_trust: local_candidate`; it has no snapshot selector and cannot prove the candidate exists in an index, diff or MR. |

Read the returned `source` and trust label. An active policy has priority over
caller refs; a conflicting ref is an error, not permission to replace it. A
nonzero exit alone does not prove absence. Inspect an editable candidate's bytes
directly when an active policy makes `config --show` report a different source.

When no active policy or explicit policy ref is selected and the required local
candidate is confirmed missing, run ordinary `init` automatically without asking
the user. Report the repository, exact command and candidate configuration path,
preserving any explicitly selected configuration path. Automatic initialization
does not authorize `--with-checks`, rule enabling or policy adoption. Use the same
root and config bindings as the inspection:

```bash
"$QUALITYGATE_BIN" --root "$REPOSITORY_ROOT" --config "$QUALITYGATE_CONFIG" init --format json
```

Until required `init` has run successfully and the required policy is available,
stop this Qualitygate verification workflow. Do not run downstream planning or
checks to bypass the prerequisite. If `init` cannot run, fails, or leaves the
required configuration unavailable, report the initialization prerequisite and
leave verification pending. Recheck configuration availability after
initialization before proceeding. This does not stop independently authorized
implementation, repository tests or read-only discovery; their results cannot
substitute for the blocked Qualitygate verification.

An existing valid policy does not require another `init`. Report selected
malformed or unreadable configuration as an error; do not overwrite it. When
policy comes from a staged/diff/MR snapshot, a newly initialized worktree policy
must also be present in the selected index/commit before checking. If a valid
local candidate already exists but is absent from that snapshot, report the
missing inclusion instead of rerunning `init`. Do not silently stage, commit or
change the snapshot selector to satisfy this prerequisite. The selected authority
determines which configuration must exist; a local candidate cannot repair or
replace an explicit ref or active signed policy.

## Inspect configuration

These read-only examples inspect configuration after runtime startup: the active
signed package when present, otherwise the local candidate. They deliberately
do not initialize on failure. Use existing bindings or record absolute paths
again for each isolated tool call. An explicit ref or selected index/commit still
needs its own inspection as described above; see
[policy context](rule-management.md) and [operations](operations.md).

### Bash

```bash
: "${REPOSITORY_ROOT:?Set the target repository root}"
QUALITYGATE_CONFIG="${QUALITYGATE_CONFIG:-qualitygate.yaml}"
"$QUALITYGATE_BIN" --root "$REPOSITORY_ROOT" --config "$QUALITYGATE_CONFIG" config --show --format json
```

### PowerShell

```powershell
if (-not $env:REPOSITORY_ROOT) { throw 'Set the target repository root' }
if (-not $env:QUALITYGATE_CONFIG) { $env:QUALITYGATE_CONFIG = 'qualitygate.yaml' }
& $QualitygateBin --root $env:REPOSITORY_ROOT --config $env:QUALITYGATE_CONFIG config --show --format json
if ($LASTEXITCODE -ne 0) { throw 'Configuration inspection failed' }
```

### Git Bash/MSYS2

```bash
: "${REPOSITORY_ROOT:?Set the target repository root}"
QUALITYGATE_CONFIG="${QUALITYGATE_CONFIG:-qualitygate.yaml}"
qualitygate_native_root="$(cygpath -m "$REPOSITORY_ROOT")" || exit 2
MSYS2_ENV_CONV_EXCL=QUALITYGATE_BUILTIN_RULES_DIR MSYS2_ARG_CONV_EXCL='*' \
  "$QUALITYGATE_BIN" --root "$qualitygate_native_root" --config "$QUALITYGATE_CONFIG" config --show --format json
```

## Preflight the intended snapshot

Identify the repository, selector, profile, task and policy source. For code
work without a separate task contract, use the final worktree and existing
repository policy. Preserve caller-selected `--staged`, `--diff` or `--mr`
instead when those define delivery. Read [operations](operations.md) for
snapshot limits, trimming and trusted evidence. Announce configured command
execution and its network boundary; do not invent tasks or trust inputs.

Run `doctor` with the same supported selector/profile/task/policy and budgets
as the intended check. Default doctor is static; select `--probe-tools` only
when declared tool execution is authorized. MR doctor is not supported;
retain that preflight gap for an MR check rather than substituting a worktree
doctor as evidence for the MR. A request only for doctor ends at its result.

With `doctor.summary.v1`, prefer `--summary --format json`. The bounded summary
identifies the full JSON report by `report.path`, SHA-256 and byte count. Read
that report for diagnostics and next-step argv; retain it through diagnosis.
`details_truncated` concerns the summary, not evidence loss from the saved
report. Exit `2`, including `report.write_failed`, remains blocked/incomplete.
Repair prerequisites within scope. A preflight pass is not delivery acceptance.

```bash
"$QUALITYGATE_BIN" --root "$REPOSITORY_ROOT" --config "$QUALITYGATE_CONFIG" doctor --worktree --profile full --summary --format json
```

## Final check and repair feedback

Use `--profile quick` or `--path` while editing. After the last change to checked
inputs, run an unfiltered `check --profile full` on the final selected snapshot.
Add required task, policy, trust, evidence and budget options consistently;
preserve an explicit `--output-dir` when durable evidence was requested.

```bash
"$QUALITYGATE_BIN" --root "$REPOSITORY_ROOT" --config "$QUALITYGATE_CONFIG" check --worktree --profile full --format json
```

Ensure all build, tests and analysis required by the repository run, whether
selected in policy or required separately. The CLI report covers only selected
checks. Completion requires all of the following:

- `profile: full`, with `scope: delivery` or `scope: task`, never `scope: path`;
- empty `plan.pending_delivery_checks`;
- `gate.complete: true`, `gate.decision: pass` and exit code `0`;
- all separately required repository checks pass for the delivered inputs.

Retain the report, snapshot content and verification digests, policy/task
digests, logs and incomplete reasons. Inspect `selection.mode`,
`selection.exclude`, `selection.excluded_paths` and `selection.empty_delivery`.
An empty or excluded delivery does not establish that all repository code is
clean. The selector derives delivery scope; the profile selects checks. There
is no separate scope selector.

| Exit | Meaning | Response |
| --- | --- | --- |
| `0` | Complete and passing for the reported scope | Apply the full completion contract above; preserve warnings and scope limits. |
| `1` | Complete with a blocking violation | Repair within authorization; never weaken policy just to pass. |
| `2` | Incomplete execution or evidence | Identify the missing prerequisite or evidence; do not claim a pass. |

Use [Agent feedback](agent-feedback.md) for bounded repair context. If inputs
change, capture and check the new snapshot; keep prior reports as history rather
than evidence for new bytes. Missing runtime, policy, project tools or required
task evidence leaves the code task verification-blocked. State what did run and
what remains unknown. A prompt review, schema-valid candidate or manual
acknowledgement does not repair a missing check or authorize policy adoption.
