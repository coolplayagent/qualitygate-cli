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

Worktree, staged, diff and MR selectors determine the delivery automatically.
With no selector, the CLI uses the worktree comparison.
It selects findings on changed lines, with changed-file selection for findings
without line positions. Build/test commands retain unexcluded dependency context;
unlocated failures, timeouts and missing evidence still affect the gate.
The profile selects checks, independently of the selected snapshot.
`--diff` and `--mr` acquire unexcluded base/head inputs before computing changes.
A per-file acquisition error is not a rule violation. Rule path filters and
severity changes cannot fix it. Use reviewed policy `exclude` entries or the
suggested `--snapshot-max-file-mib` within its supported maximum; increasing only
the total budget is insufficient.
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

## Existing-repository trimming

Use this workflow for first-time adoption or explicitly requested exclusion of
historical resources. Use the matching Skill/runtime; an old executable rejecting
`exclude` is a compatibility gap, not permission to omit it. Verify that reports
contain delivery selection evidence before claiming the requested boundary.

1. Identify the repository, selector, policy source and intended delivery. Use
   `init --with-checks` only when candidate command adoption is authorized;
   existing configurations are preserved, not augmented automatically.
   Otherwise, when configuration is missing, run ordinary `init` automatically
   without asking. When initialization is required, stop here until it succeeds
   and the required policy is available. Follow the
   [initialization prerequisite](../SKILL.md#initialization-prerequisite).
2. Read `snapshot_preflight`: large/unsupported entries, excluded paths, truncation
   and incomplete reasons. This is HEAD/worktree metadata, not proof that a chosen
   MR, index, arbitrary refs or the build tools are ready.
3. Review precise exclusion candidates: show matched paths and explain why they
   are historical resources, including possible build/test dependencies. Prefer
   exact files or the smallest useful directory. Do not automatically exclude all
   large files or an entire legacy tree to obtain a pass.
4. With existing authorization for adoption and trimming, edit the candidate
   directly within that scope; do not request repeated approval for routine edits.
   Without that authorization, present a concrete proposed diff. Example:

   ```yaml
   exclude:
     - "gitbook/images/**"
     - "legacy/demos/**"
   ```

   Patterns are case-sensitive repository-relative globs with `/` separators.
   They affect tracked files too. No absolute paths, parent traversal, negation
   (`!`) or `.qualitygateignore` are supported. `.gitignore` alone does not remove
   tracked snapshot inputs. Excluded content is not read, checked or materialized
   for tools. Effective policy, tasks, custom rules, their sources and protected
   verification assets cannot be excluded.
5. Run `init --format json` again and inspect `excluded_file_count`,
   `excluded_paths`, remaining oversized entries and `complete`. Keep the candidate
   command review separate: `init --with-checks` does not install or execute tools.
6. Run the intended final check, for example:

   ```bash
   qualitygate check --worktree --profile full --format json
   qualitygate check --mr https://github.com/owner/repo/pull/123 --profile full --format json
   ```

   For staged/diff/MR, place the configuration in the selected index/commit first.
   A dirty local exclusion cannot alter a remote MR's policy. Respect any selected
   policy reference or active signed policy; a candidate edit does not replace it.
7. If exclusion removes a required resource, restore/narrow that exclusion and
   rerun. Preserve missing-dependency, build, test, timeout and evidence errors;
   never disable checks, relax severity or expand exclusions merely to make them
   disappear. Unexcluded historical inputs still obey acquisition limits.
8. Report the selector and commits, effective scope, exclusion patterns and matched
   paths, execution-context digest, checks, pending requirements and final result.
   Say "delivery-scope verification passed" when appropriate; do not claim full
   repository coverage. An empty or entirely excluded delivery must be explicit.

Example outcomes: excluding a historical demo permits acquisition; excluding a
resource used by the build produces a real failure and requires narrowing the
pattern; editing local YAML without committing it does not fix an MR capture.

## Commands and mutations

For repository verification, inspect configuration first; do not batch this
prerequisite with later CLI commands:

```bash
qualitygate --root "$REPOSITORY_ROOT" config --show --format json
```

If configuration is missing, run ordinary `init` automatically without asking,
reporting the exact command, repository and candidate configuration path. Preserve
any explicitly selected configuration path. Until initialization succeeds and the
required policy is available, stop subsequent Qualitygate workflow actions,
including rule discovery, planning and checks. If initialization cannot run,
fails, or leaves configuration unavailable, report the prerequisite and stop.
Report malformed or unreadable configuration as an error instead of overwriting
it. Do not substitute repository tests or another review unless separately
requested. See the [initialization prerequisite](../SKILL.md#initialization-prerequisite)
for selected-snapshot requirements. Recheck configuration availability after
initialization; once the prerequisite is satisfied, read-only rule discovery
can continue:

```bash
qualitygate --root "$REPOSITORY_ROOT" rules list --format json
```

Ordinary `qualitygate init` creates a missing candidate config automatically.
`qualitygate rules enable <id>` changes a candidate config and still requires
explicit user authorization, as do command adoption with `init --with-checks`
and policy adoption. Creating or editing a candidate does not establish team
adoption or release a change.

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

`snapshot.content_digest` identifies the captured execution files.
`snapshot.verification_digest` additionally binds delivery/repository scope,
exclusions, changes and any path filter. Retain both in evidence; signed snapshot
bindings must include the verification digest when present. A matching content
digest alone cannot authorize evidence produced for another scope. Protected
policy comparisons capture each policy's exclusions independently against the
same Git revisions, and bind both input identities when their contents differ.

## Result handling

Keep the report, snapshot digest, policy digest, task digest, execution logs,
and incomplete reasons together. Exit status has fixed meaning:

| Status | Meaning | Agent response |
| --- | --- | --- |
| `0` | complete and passing | report the selected scope and retained evidence |
| `1` | complete with a blocking violation | report the violation; do not silently weaken policy |
| `2` | incomplete | identify missing evidence or prerequisites; do not claim pass |

For code-task completion, require an unfiltered `full` report for the final
snapshot with `scope: delivery`, `scope: repository` or `scope: task`, an empty
`plan.pending_delivery_checks`, `gate.complete: true` and `gate.decision: pass`.
Quick or path-scoped exit status `0` covers only its selected feedback scope.
If checked inputs change after the full run, select the new snapshot and rerun.
Report remaining warnings and the verification boundary alongside the pass.

Use a repair loop only when the user authorizes code or policy changes. After a
repair, rerun against a newly selected snapshot and retain both reports rather
than treating a prior report as evidence for changed inputs.
