# Qualitygate operational boundaries

Read this reference when the request requires more than listing rules or
showing the effective configuration.

For the check's configuration and repair evidence, read
[file contracts](file-contracts.md) when bounding instruction files or checking
required paths, and [diagnostic ratchets](diagnostic-ratchets.md) when preventing
analyzer debt growth. Both use the snapshot and policy boundaries below.

## Snapshot and policy selection

Choose exactly the selector the user requested:

- `--worktree` checks captured current files, including uncommitted changes.
- `--staged` checks index bytes.
- `--diff <base>..<head>` checks a named comparison.
- `--mr <URL>` resolves a GitHub or GitLab merge-request comparison.

Do not substitute one selector for another. Use `--path` only for explicitly
scoped feedback, and do not describe it as full delivery validation. Keep
`--profile quick` and `--profile full` distinct for the same reason.

`--path` can be combined with `--staged`, `--worktree`, or `--diff`; it filters
feedback while retaining the complete snapshot for policy and build inputs.
For large repositories, the default acquisition budget is 256 MiB per tree,
100,000 files, 2 MiB per file, four concurrent content readers and 120 seconds.
Git content is size-checked and acquired in batches of at most 4 MiB and 1,024
objects; explicitly allowed larger files use singleton batches up to 8 MiB.
The runner's 16 MiB per-stream limit remains unchanged. Caller controls
are `--snapshot-max-mib` (1–1024), `--snapshot-max-file-mib` (1–8, default 2), `--snapshot-jobs` (1–16) and
`--snapshot-timeout-secs` (1–3600). Choose them for available memory and retain
budget errors as incomplete execution. Recheck commands preserve these options.

Test-effectiveness overlays retain the selected per-file capacity and complete
historical inputs. Protected candidate validation instead uses the external
suite's `budget.snapshot_max_file_mib` (default 2, range 1–8), bound to the suite
digest and its external authorization. Ordinary check flags do not override it.

`--diff` and `--mr` retain complete base/head trees before computing changes.
A per-file acquisition error is not a rule violation: neither rule path filters
nor severity changes can fix it. Follow the suggested `--snapshot-max-file-mib`
within its supported maximum; increasing only the total budget is insufficient.
`init` includes advisory HEAD/worktree metadata preflight, counts and bounded
details for large/unsupported entries, plus actionable next steps. Its success
means a candidate was handled, not that snapshot acquisition or tools passed.
Human guidance escapes control characters in repository paths; JSON keeps the
original path value for machine consumers.
Respect the user's existing adoption authorization; capacity retries do not
mutate repository policy. Do not downgrade incomplete acquisition to a warning
and claim a complete gate.

```bash
qualitygate check --root "$REPOSITORY_ROOT" --staged --path src --profile quick --snapshot-jobs 4
```

The configuration, task, and custom rules come from the selected snapshot
unless the caller supplies `--policy-ref`. A policy reference resolves to a
commit, but it is not proof of approval. Never guess a protected reference or
replace a caller's selected reference with the current branch.

## Commands and mutations

These commands do not modify a repository policy:

```bash
qualitygate --root "$REPOSITORY_ROOT" rules list --format json
qualitygate --root "$REPOSITORY_ROOT" config --show --format json
```

`qualitygate init` may create a new candidate config, and `qualitygate rules
enable <id>` changes a candidate config. Use them only after explicit user
authorization. Both are candidate-policy operations; neither establishes team
adoption or releases a change.

`qualitygate check` materializes a selected Git snapshot and may run configured
project tools. Before running it, report the repository root, selector, profile,
task, policy reference, and whether it can access networked tools. Preserve an
explicit `--output-dir` when the user needs durable evidence; otherwise allow
the CLI's bounded default evidence location.

## Tasks, trust, and manual evidence

For a task that requires trusted policy or manual evidence, keep trust inputs
outside the checked repository and pass them explicitly:

```bash
qualitygate check --root "$REPOSITORY_ROOT" --worktree --profile full \
  --task tasks/request.yaml --policy-ref "$TRUSTED_COMMIT" \
  --trust-store /trusted/qualitygate/trust.json \
  --evidence-dir /trusted/qualitygate/records --format json
```

Do not create, sign, refresh, or reinterpret a manual approval. Missing, stale,
foreign, expired, or malformed evidence is incomplete validation. A local
configuration or a caller-supplied ref is not an approval decision.

## Result handling

Keep the report, snapshot digest, policy digest, task digest, execution logs,
and incomplete reasons together. Exit status has fixed meaning:

| Status | Meaning | Agent response |
| --- | --- | --- |
| `0` | complete and passing | report the selected scope and retained evidence |
| `1` | complete with a blocking violation | report the violation; do not silently weaken policy |
| `2` | incomplete | identify missing evidence or prerequisites; do not claim pass |

For code-task completion, require an unfiltered `full` report for the final
snapshot with `scope: repository` or `scope: task`, an empty
`plan.pending_delivery_checks`, `gate.complete: true` and `gate.decision: pass`.
Quick or path-scoped exit status `0` covers only its selected feedback scope.
If checked inputs change after the full run, select the new snapshot and rerun.
Report remaining warnings and the verification boundary alongside the pass.

Use a repair loop only when the user authorizes code or policy changes. After a
repair, rerun against a newly selected snapshot and retain both reports rather
than treating a prior report as evidence for changed inputs.
