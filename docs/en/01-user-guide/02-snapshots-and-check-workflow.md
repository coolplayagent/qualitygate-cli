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
checks. Only an unfiltered `full` report whose scope is `delivery` or `task`,
whose pending delivery list is empty, and whose gate is complete can establish
the configured delivery decision.

## Large repositories

The worktree, staged, diff or MR selector determines the delivery automatically;
without a selector the CLI compares the worktree. Quick/full selects checks.
Line findings select changed lines; findings without line positions select changed
files, including deletions, renames, binary and mode changes. Unlocated command/test
failures, timeouts and missing evidence remain gate inputs. There is no separate
scope option. An empty delivery is not repository approval.

Builds and tests retain unexcluded dependency context. Diff/MR acquire both sides
of that context; rule language/path filters alone do not omit content. Configure
reviewed exclusions in the effective `qualitygate.yaml`:

```yaml
exclude:
  - "gitbook/images/**"
  - "legacy/demos/**"
```

These case-sensitive repository-relative globs use `/` and also match tracked files.
Absolute paths, parent traversal, negation and `.qualitygateignore` are unsupported.
Excluded content is not acquired, checked or materialized for commands. Policy/task
files, custom rules, sources and protected verification assets cannot be excluded.
If tools need an excluded resource, narrow the exclusion and rerun. `.gitignore`
does not remove tracked inputs. Diff/MR/index use their selected policy source,
never unrelated dirty worktree exclusions.

Unexcluded files retain a default 2 MiB capacity; choose `--snapshot-max-file-mib N`
(1-8) to retain larger inputs. Above 8 MiB, acquisition remains incomplete unless
the effective policy explicitly excludes the resource. Increasing the total
`--snapshot-max-mib` alone does not change the single-file limit.
Reports retain scope, exclusions, changed files/line counts, empty delivery and
the execution-context digest in `selection`. Scope and exclusions bind verification
identity; rechecks retain scope and capacity. A delivery pass is not repository-wide
approval.

Test-effectiveness checks use the same per-file capacity when composing old
production code with new tests, retaining effective exclusions and context.
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
