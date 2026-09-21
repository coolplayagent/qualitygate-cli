# Snapshots and the check workflow

[简体中文](../../zh/01-user-guide/02-snapshots-and-check-workflow.md) · [Volume index](README.md)

Choose one selector that matches the delivery surface:

```bash
qualitygate check --staged --profile full --format json
qualitygate check --worktree --profile quick --format json
qualitygate check --diff BASE..HEAD --profile full --format json
qualitygate check --mr https://github.com/owner/repository/pull/123 --format json
```

- `--staged` reads index bytes, not the working-tree copy.
- `--worktree` captures the current worktree plus eligible untracked files.
- `--diff` compares two resolved commits.
- `--mr` resolves a GitHub or GitLab merge request and records the actual base,
  head, merge base, provider, and acquisition evidence.

MR access uses caller-provided credentials and endpoints. Authentication,
network, ref drift, rate limits, and unsupported provider responses are
incomplete execution, never a clean result.

## Plan, execute, decide

The CLI loads strict configuration from the selected snapshot (or an explicit
policy reference), validates an optional task contract, maps changed files and
project facts, then builds a dependency-aware plan. Commands run in a disposable
materialization of that snapshot. Their stdout, stderr, status, duration,
version, and report artifacts are captured within configured bounds.

`quick` and `--path` deliberately omit work and list the omitted delivery
checks. Only an unfiltered `full` report whose scope is `repository` or `task`,
whose pending delivery list is empty, and whose gate is complete can establish
the configured delivery decision.

## Large repositories

`--diff` and `--mr` acquire complete base and head trees before mapping changes.
`--path` filters feedback after acquisition. Language selection and rule path
filters do not exclude snapshot inputs. A historical file can therefore cause
an acquisition error even when the only change is a README outside its path.
This is `incomplete` (exit 2), not a repository-scope rule violation.

The default per-file budget is 2 MiB. For a reviewed larger input, use
`--snapshot-max-file-mib 8` (supported range 1–8), retaining its complete bytes
and snapshot identity. `--snapshot-max-mib` controls the separate total budget;
raising it alone cannot resolve a per-file error. Init preflight and errors
suggest a sufficient file budget when supported. Files above 8 MiB remain
unsupported; no implicit ignore, severity downgrade or partial passing gate is
introduced. Recheck commands retain the selected per-file budget.

Test-effectiveness checks use the same per-file capacity when composing old
production code with new tests; unrelated historical files remain complete.
For protected `policy candidate validate`, set `budget.snapshot_max_file_mib`
in the external acceptance suite instead (default 2, range 1–8). Both policies
use that bound. Changing it changes the suite digest and requires matching
external trust; ordinary check flags cannot override protected suite budgets.

Snapshot acquisition checks object sizes before reading contents and uses
bounded batches and concurrency. Callers can tune the total byte, worker, and
deadline budgets without changing policy meaning:

```bash
qualitygate check --worktree --profile full \
  --snapshot-max-mib 512 --snapshot-jobs 8 --snapshot-timeout-secs 300
```

Symlinks, submodules, unresolved index conflicts, oversized files, missing Git
objects, and budget exhaustion fail closed. A path filter narrows feedback but
does not redefine the underlying policy or turn a partial run into full
delivery evidence.

Retain the JSON report and referenced evidence together. If checked files,
policy, task input, or reports change, select and check a new snapshot.
