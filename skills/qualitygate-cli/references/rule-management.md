# Progressive discovery and candidate configuration

Read-only commands work before `init` and do not change rule selection:

```bash
qualitygate rules categories --format json
qualitygate rules list --category test --language rust --source builtin --format json
qualitygate rules describe test-naming --format json
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
forced removal restores origin-based defaults. If that starter origin was also
deleted, the rule has no category until reassigned.

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
Project rule reads and schema parsing use at most eight workers, with 256 files,
1 MiB content and 4,096 directory-entry limits. Catalog errors and timeouts remain
errors even when the selected category would otherwise be empty.
