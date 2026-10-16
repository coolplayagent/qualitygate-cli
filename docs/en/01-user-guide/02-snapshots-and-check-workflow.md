# Snapshots and the check workflow

[简体中文](../../zh/01-user-guide/02-snapshots-and-check-workflow.md) · [Volume index](README.md)

## Understand the inputs

| Concept | What it fixes |
| --- | --- |
| Snapshot | The exact captured files and Git change selection used for execution. |
| Policy | Rules, command checks, severity and profiles selected for this run. |
| Task contract | Additional acceptance items for a specific request, when supplied. |
| Evidence | Results and artifacts tied to those inputs; an edit requires new verification. |

`quick` and `full` choose checks. A snapshot selector chooses the bytes and
changes being verified. A full check therefore does not automatically mean
repository-wide findings: the CLI evaluates delivery selected by Git changes.

## Select the snapshot

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

## Read the report as evidence

| Field | Read it for |
| --- | --- |
| `snapshot` and `policy` | Which captured inputs and policy produced the result. |
| `selection` | Changed files/lines, exclusions, empty delivery and execution context. |
| `profile` and `scope` | Which checks ran and whether this is delivery, task or path feedback. |
| `plan.pending_delivery_checks` | Work still required for a delivery decision. |
| `gate.complete` and `gate.decision` | Whether validation finished and whether blockers remain. |

Line findings apply to changed lines; findings without line positions apply to
changed files, including deletions, renames, binary and mode changes. Unlocated
command failures, timeouts and missing evidence remain gate inputs. An empty
delivery does not prove repository-wide quality.

Retain the JSON report and referenced evidence together. Content identity and
verification scope are distinct; see [evidence bindings](../02-reference/04-policy-task-and-signed-evidence.md).
If files, policy, task or report inputs change, capture and check a new snapshot.

## Work with large repositories

Builds and tests need unexcluded dependency context, even for a small diff.
Language selection or `--path` does not remove those inputs. Start with `doctor`
for your actual selector; its metadata identifies capacity or unsupported-file gaps.

1. Keep files needed by checks and choose a supported acquisition budget when justified.
2. Review precise policy exclusions for unrelated resources; they also disappear from command inputs.
3. Repeat doctor and the final full check with the same selector and budget.

For example, if the report calls for 4 MiB per file:

```bash
qualitygate doctor --worktree --profile full --snapshot-max-file-mib 4 --format json
qualitygate check --worktree --profile full --snapshot-max-file-mib 4 --format json
```

Exclusion syntax, protected files and selected-policy behavior are defined in
[runtime and preflight](../02-reference/06-runtime-and-preflight.md).
[Resource limits](../02-reference/05-exit-codes-and-resource-limits.md) lists all
budget options. Increasing capacity changes acquisition, not policy approval.


---

[Previous: Installation and first check](01-installation-and-first-check.md) · [Contents](../README.md) · [Next: Rule configuration](03-rule-configuration.md)
