# Progressive discovery and candidate configuration

Read-only commands work before `init` and do not change rule selection:

```bash
qualitygate rules categories --format json
qualitygate rules list --category test --language rust --source builtin --format json
qualitygate rules describe test-naming --format json
qualitygate rules context --category test --policy-ref HEAD --format json
```

For an authorized candidate-policy change, use validated CLI mutations:

```bash
qualitygate rules categories create migration --description "API migration" --format json
qualitygate rules assign import-boundary --category migration --format json
qualitygate rules categories rename migration api-migration --format json
qualitygate rules configure import-boundary --param 'forbidden_imports.rust=["^crate::interfaces"]' --format json
qualitygate rules enable import-boundary --format json
qualitygate rules disable import-boundary --format json
qualitygate rules categories delete api-migration --force --format json
```

Use only the caller's actual import boundary. Categories are mutable labels;
assignment does not activate a rule. Starter names are core, test, security,
architecture, style and project. Rename preserves starter identity and moves
assignments atomically. Delete needs `--force` when explicit assignments remain;
forced removal removes only that category's memberships. Repeated assignments
add categories, and `rules unassign <id> --category <name>` removes one. Removing
the last explicit membership restores the origin default; if that origin was
deleted, the rule has no category until reassigned. Rule rows expose the full
`categories` array and a legacy first `category`.

Filtered reads always retain `mandatory` rules and `mandatory_checks` separately
from selected rows. Use `rules context --policy-ref <caller-selected Git ref>`
to freeze the baseline; its exact `policy_digest` and trust label accompany the
result. Omitting the ref reads the local candidate and cannot establish trusted
policy selection. `rules categories list` is also accepted.

Describe exposes parameter types, descriptions, defaults and schema fragments.
`--param key=JSON` requires JSON values, including quoted strings. Dotted paths
retain sibling object values; an undotted object replaces the entire parameter.
Repeat `--param` for one atomic edit. `configure` also accepts `--severity` and
`--required true|false`; new configured rules start disabled. Project DSL
assertions are managed in rule definitions, not through parameters.

All mutations require an existing candidate config and support `--config`.
They preserve unrelated settings and inherited defaults, validate before atomic
replacement, preserve file permissions and reject conflicting writers. A killed
writer leaves `<config>.lock`; verify that it has stopped before explicitly
recovering the lock. Retain mutation JSON (operation, changed flag, before/after
digests and result) with the command log and reviewed Git diff. Changed YAML is
normalized; semantic no-ops preserve original bytes.

Source reviews and policy trust remain separate. Executable changes can make
existing source-review bindings stale; check reports retain missing/stale
evidence. Neither a category operation nor configuration success approves a
policy or establishes a passing gate.

Discovery reads policy/rule directories without scanning repository sources.
Once an evidence/policy archive exists, semantic commands require an explicit
candidate ID. Use `policy candidate create` with the caller-selected parent,
actor, reason and retained evidence, followed by `policy candidate rules`.
After signed promotion, `rules context`, inventory and ordinary checks select
the immutable active package; category edits cannot weaken mandatory rules.

For an authorized policy evolution workflow, use externally supplied suite,
trust and approval files. External protected files and retained logs accept
native absolute paths on Windows and POSIX; they must remain outside the
checked repository, without traversal or symlink ancestors, within read limits.
Repository-relative rule paths still require forward slashes.
Snapshot materialization and input verification each use at most four I/O
workers per executing check within their 30-second budget. A timeout or changed
input remains incomplete; reducing the checked inventory is not a repair.
`policy candidate validate` compares pinned replay,
held-out and anchor tasks with bounded parallelism. `policy candidate
approval-subject` produces the exact signing subject, `approve` verifies the
independent human signature, and `promote` rechecks it before activation.
Never manufacture an approval or private key. Missing tools/evidence and
timeouts remain incomplete. `policy rollback-subject` and `policy rollback`
require a distinct external signature and preserve the historical transition.

`rules revalidate`, `demote`, `deprecate`, `retire` and `revoke` take
`--candidate`, `--actor` and `--reason`; they prepare candidate changes and do
not grant approval. `rules history` exposes evidence and lifecycle states.
`policy effectiveness` separates oracle validity, observed rule/check use and
runtime from unknown downstream benefit, context tokens and review effort.
Only results with matching suite, parent, evaluator epoch/digest, environment,
budget and job count share a longitudinal group.
Project rule reads and schema parsing use at most eight workers, with 256 files,
1 MiB content and 4,096 directory-entry limits. Catalog errors and timeouts remain
errors even when the selected category would otherwise be empty.
