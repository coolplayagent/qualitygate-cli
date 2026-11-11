# Progressive discovery and candidate configuration

Built-in discovery works without a repository configuration and does not change
rule selection. Use the selected `--root` and `--config` consistently:

```bash
qualitygate rules categories --format json
qualitygate rules list --category test --language rust --source builtin --format json
qualitygate rules describe test-naming --format json
```

Reading a frozen policy context also requires a Git reference containing the
selected configuration; an absent configuration at `HEAD` is an error:

```bash
qualitygate rules context --category test --policy-ref <selected-ref> --format json
```

[Repository verification](verification.md) owns the missing-configuration and
initialization workflow. Discovery does not bypass that prerequisite for checks.

For a candidate-policy change covered by the task's existing authorization, use
validated CLI mutations:

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
from selected rows. Context uses this policy precedence:

- An active signed policy selects the immutable active package, including when
  `--policy-ref` is omitted. A supplied ref must match the active reference;
  historical replay belongs to candidate validation.
- Without an active policy, `--policy-ref <selected-ref>` reads the configuration
  and rules from that commit and reports `caller_supplied_ref` trust.
- Without either, context reads the local candidate with `local_candidate` trust.

Read the returned `policy_digest` and trust label. A caller-selected ref alone
is not independent approval. `rules categories list` is also accepted.

`rules describe` exposes parameter types, descriptions, defaults and schema
fragments; language descriptors also expose whether the effective parameter is
required. A fixed scope requires the declared list; a subset scope accepts only
a nonempty subset of declared languages. Omit an override to inherit its default,
rather than using an empty list to bypass scope. Schema string-length checks do
not replace UTF-8 byte budgets, Rust regex validation or other semantic checks.
`--param key=JSON` requires JSON values, including quoted strings. Dotted paths
retain sibling object values; an undotted object replaces the entire parameter.
Repeat `--param` for one atomic edit. `configure` also accepts `--severity` and
`--required true|false`; new configured rules start disabled. Project DSL
assertions are managed in rule definitions, not through parameters. Built-in
`diagnostic` message/fix metadata belongs to the shipped rule definition and
changes presentation only; it is not a configurable policy parameter or verdict.

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
Once an evidence/policy archive exists, semantic policy edits require an explicit
candidate ID. Use `policy candidate create` with the caller-selected parent,
actor, reason and retained evidence, followed by `policy candidate rules`.
After signed promotion, `rules context`, inventory and ordinary checks select
the immutable active package; category edits cannot weaken mandatory rules.

For an authorized policy evolution workflow, use externally supplied suite,
trust and approval files. External protected files and retained logs accept
native absolute paths on Windows and POSIX; they must remain outside the
checked repository, without traversal or symlink ancestors, within read limits.
Repository-relative rule paths still require forward slashes.
See [operations](operations.md) for snapshot materialization, external temporary
storage and execution budgets. Missing or changed inputs remain incomplete;
reducing the checked inventory is not a repair.
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
