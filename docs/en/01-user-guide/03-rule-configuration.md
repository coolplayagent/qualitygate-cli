# Rule configuration

[简体中文](../../zh/01-user-guide/03-rule-configuration.md) · [Volume index](README.md)

A repository policy chooses which rules and commands matter for your delivery.
Start with the candidate from `init`; inspect it before changing selection or
thresholds. The examples below run from the repository root.

## 1. Inspect the catalog and effective policy

```bash
qualitygate rules categories --format json
qualitygate rules list --source all --format json
qualitygate rules describe line-ending --format json
qualitygate config --show --format json
```

The catalog includes unselected built-in packages and project rules. Read each
rule's `enabled`, `source`, `language`, capabilities, severity and parameters.
Listing or assigning categories does not enable checks. Compare catalog entries
with the effective policy to see what a check will actually use.

## 2. Choose the mechanism

| Repository requirement | Configure |
| --- | --- |
| A supported commit, text or syntax convention | An existing built-in rule, after reading its definition. |
| A source-bound file, name, marker or bounded assertion | A project rule under `qualitygate/rules`. |
| A lint, test or coverage command | A command check with the appropriate report adapter. |
| Existing analyzer debt must not grow | A report ratchet with a fresh baseline run. |
| A person must judge an acceptance item | Explicit manual acceptance and external signed evidence. |

File contracts and report ratchets are configuration patterns, not extra
built-in rule IDs. Missing semantic facts remain gaps; a text match cannot prove
call resolution, data flow or runtime correctness.

## 3. Change and review a candidate

For example, selecting the existing line-ending rule is an explicit mutation:

```bash
qualitygate rules enable line-ending
qualitygate config --show --format json
```

Use `rules configure RULE_ID --set key=value` only with parameters supported by
that rule's definition. Writes validate the whole candidate before replacing it
atomically. Invalid parameters, duplicate IDs and unreadable packages are errors.
A valid candidate still needs the team's normal policy review and adoption.
Disabling a required check to make a failing change pass is not a repair.

For project rules, continue with [Schema-guided repository rules](06-schema-guided-repository-rules.md).
For command reports and adoption boundaries, use the
[report reference](../02-reference/03-projects-and-tool-reports.md) and
[policy lifecycle](../02-reference/04-policy-task-and-signed-evidence.md).

## 4. Verify the selected policy

```bash
qualitygate doctor --worktree --profile full --format json
qualitygate check --worktree --profile full --format json
```

Confirm the intended checks appear in the plan and inspect pending work,
violations and incomplete evidence. A policy or rule change requires new
verification; staged/diff checks must contain the reviewed configuration in their
selected snapshot. Preserve the report with the adopted policy identity.

---

[Previous: Snapshots and the check workflow](02-snapshots-and-check-workflow.md) · [Contents](../README.md) · [Next: Schema-guided repository rules](06-schema-guided-repository-rules.md)
