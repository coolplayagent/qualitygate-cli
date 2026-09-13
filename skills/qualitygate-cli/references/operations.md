# Qualitygate operational boundaries

Read this reference when the request requires more than listing rules or
showing the effective configuration.

## Snapshot and policy selection

Choose exactly the selector the user requested:

- `--worktree` checks captured current files, including uncommitted changes.
- `--staged` checks index bytes.
- `--diff <base>..<head>` checks a named comparison.
- `--mr <URL>` resolves a GitHub or GitLab merge-request comparison.

Do not substitute one selector for another. Use `--path` only for explicitly
scoped feedback, and do not describe it as full delivery validation. Keep
`--profile quick` and `--profile full` distinct for the same reason.

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

Use a repair loop only when the user authorizes code or policy changes. After a
repair, rerun against a newly selected snapshot and retain both reports rather
than treating a prior report as evidence for changed inputs.
