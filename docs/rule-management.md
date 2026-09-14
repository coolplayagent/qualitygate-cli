# Rule configuration and progressive discovery

[Issue #3](https://github.com/coolplayagent/qualitygate-cli/issues/3) adds dynamic
categories and validated candidate configuration changes. Every command supports
global `--root`, `--config` and `--format json|table|markdown` options.

```bash
qualitygate rules categories --format json
qualitygate rules categories create migration --description "API migration rules" --format json
qualitygate rules assign import-boundary --category migration --format json
qualitygate rules list --category migration --language rust --source builtin --format json
qualitygate rules describe import-boundary --format json
qualitygate rules configure import-boundary --param 'forbidden_imports.rust=["^crate::interfaces"]' --format json
qualitygate rules enable import-boundary --format json
qualitygate rules disable import-boundary --format json
qualitygate rules categories rename migration api-migration --format json
qualitygate rules categories delete api-migration --force --format json
```

## Category semantics

The starter registry contains `core`, `test`, `security`, `architecture`,
`style` and `project`. Core rules use `core`; test naming, parameterization and
traceability use `test`; security patterns use `security`; import/module and
dependency rules use `architecture`; comment language and TODO markers use
`style`; project-authored definitions use `project`. These are discovery labels;
they do not change package selection, severity, execution order or activation.

`rules categories` returns each name, description, rule count, enabled count and
`custom` flag without including rule definitions. Counts use one definition per
ID, preferring the active definition, then a discovered project definition.
`rules list` retains both origins for a project override and combines category,
source and language filters. Unknown categories fail instead of returning a
misleading empty inventory. Empty existing categories return an empty list.

Without a `categories` field, a policy inherits the starter registry. Category
creation, rename and deletion materialize the complete registry in
`qualitygate.yaml`. `rule_categories` maps rule IDs to explicit single-category
assignments, including unselected rules. The optional category `origin` retains
the starter identity across renames, so implicit assignments follow the rename
and `custom` remains false. Newly created categories have no origin and report
`custom: true`. Names use the existing 1–128 byte rule-ID alphabet: ASCII
letters, digits, hyphen, underscore and dot. Descriptions allow at most 1,024
bytes without control characters. Limits are 256 categories and 4,096 explicit
assignments, within the existing 1 MiB policy limit.

Rename updates explicit assignments and the registry in one transaction.
Delete refuses explicit assignments unless `--force` is supplied; forced
deletion removes those assignments so rules return to their origin category's
current name. Starter categories are also mutable. When their origin has been
deleted, affected rules have `category: null` until explicitly reassigned.
Deleted starter categories are not silently recreated; creating a new custom
category with the same name does not reclaim that origin.

Read-only discovery works before `init`, including `qualitygate/rules` when
present. An explicit `custom_rules` directory retains precedence. Mutations
require an existing candidate configuration. Assigning a discovered project
rule does not select its package. Activating/configuring a previously unselected
language rule selects its language package; a new configured rule starts
disabled until `rules enable`. Managing a new project rule selects its discovered
directory and retains the existing source-review evidence requirements at check time.

## Parameter discovery and validation

`rules describe <id>` includes ID, definition version, description, category,
implementation, language scope, required capabilities, defaults, effective
configuration, enabled state, standard references and the complete definition.
Each accepted parameter has a name, type, description, default, `has_default`
indicator and a JSON Schema fragment. Missing defaults are reported as null
with `has_default: false`. Project DSL assertions remain in their definitions;
they expose no configurable parameters. For duplicate IDs, describe uses the
active definition, then an available project definition.

Values passed to `--param key=JSON` must be JSON, including quotes around string
values. Shell quotes protect those JSON quotes. Repeat `--param` to edit several
parameters in one transaction. Dotted paths edit object members and preserve
siblings, including inherited defaults in that object. An undotted object value
replaces that entire parameter. Arrays are replaced as a whole.

```bash
qualitygate rules configure test-naming --param 'patterns.rust="^test_"' --format json
qualitygate rules configure diff-size --param max_added_lines=500 --format json
qualitygate rules configure security-sensitive-api --param 'prohibited_patterns.java=["\\bRuntime\\.exec\\b"]' --format json
qualitygate rules configure test-naming --severity warning --required false --format json
```

Parameter names share the executable validator's inventory. Values pass the
existing typed and semantic checks, including regular-expression/glob validity,
language names, minimum counts, marker contracts and project dependency shapes.
Unknown names, wrong types, repeated/overlapping paths and malformed JSON fail
before writing. A transaction allows at most 128 parameter edits, 16 path
components and 1 MiB combined parameter text. Configuration revalidation also
checks profiles, prerequisites and custom rules, and recomputes source-review bindings.

`enable` adds the rule to existing quick/full profiles. `disable` retains its
settings, profile membership and dependency references; a required dependent
check cannot turn a disabled prerequisite into successful evidence. Configure
preserves the existing enabled state. Changing a bound executable setting can
invalidate its source review; the next check reports missing/stale review evidence
without inventing a replacement approval. Trusted policy and snapshot checks retain their existing
behavior after candidate edits.

## Atomic writes and evidence

Successful mutations return the operation and arguments, changed flag,
before/after SHA-256 byte digests and the affected configuration/category result.
Retain this JSON with the caller's command log and version-control diff for an
audit trail. These local candidate records do not authorize policy adoption.

All mutations validate the complete candidate and write a flushed temporary
file in the policy directory, preserve permissions and atomically replace the
policy. Unrelated explicit values and omitted defaults stay intact. A semantic
no-op retains the original bytes. A changed document uses deterministic YAML
serialization; comments and custom formatting are not retained on such writes.

A create-new `<config>.lock` serializes cooperating CLI writers; a competing
writer fails immediately. Failure removes its own lock and temporary file.
After a killed process, the lock remains for explicit recovery: establish that
the writer has stopped before removing that lock. The CLI does not break locks
based on age. Before replacement it also compares current policy bytes with the
initial read, rejecting observed external edits. Editors that ignore the lock
can still race the final comparison and rename; the lock protocol is required
for concurrent writers. Symlink policies and ancestors are rejected.

## Large-repository performance contract

Discovery reads the chosen policy and rule directories, without traversing
repository source files or running Git/build commands. A query loads each
project definition once and reuses its catalog for effective settings. Category
counts use keyed lookup and avoid serializing complete definitions. Describe
serializes only the requested rule.

Project directory discovery is bounded to 4,096 entries, 256 YAML files and
1 MiB total input. File sizes are checked before content reads, then checked
again after each read. Reading and parsing use at most eight blocking worker
threads, capped by available CPU parallelism; fewer than 16 files use the caller
worker directly. CLI orchestration remains on `spawn_blocking`. Each read or
parse stage has a cooperative 30-second deadline, checked between operations
and before returning; an individual synchronous OS read/parser call cannot be
preempted. Tasks and retained bytes stay bounded. Results are joined in sorted
path order, including failures; any worker failure discards partial results.

`config::parallel` tests prove actual overlap, the concurrency cap, ordered
results, timeout/error propagation and worker-panic handling. The project
inventory benchmark parses 256 rules with 1/4/4/1 workers, compares complete
definitions and errors and requires each pass to finish within five seconds.
The CLI integration fixture has 18,000 unrelated files and 256 project rules;
categories/list/describe each have a five-second regression threshold and the
category JSON must stay below 2 KiB. Run with `--nocapture` to retain timing.

```bash
cargo test --all-features --lib config::project_inventory -- --nocapture
cargo test --all-features --test rule_management -- --nocapture
cargo test --all-features --test large_repository -- --nocapture
```

The existing [snapshot performance contract](large-repositories.md) separately
covers complete 18,000-file trees, shared Git/worktree reader limits and
snapshot consistency. Timing thresholds detect regressions in controlled
fixtures; measurements depend on CPU, storage, cache and host contention.
Linux observations do not establish native Windows/macOS performance.

The 2026-09-14 Linux development-build measurement used 1/4/4/1 workers and
preserved identical complete results in each run:

| Workload | One worker (two runs) | Four workers (two runs) |
|---|---|---|
| Parse 256 project rules | 27.29 / 27.34 ms | 16.58 / 16.66 ms |
| Capture 18,003 files, over 35 MiB | 1.395 / 1.127 s | 0.863 / 0.868 s |

The schema compiler was warmed before the parser-only benchmark. Separate CLI
measurements include startup, reads and validation: categories took 146 ms,
category-filtered list 226 ms, and describe 128 ms in the 18,000-source-file,
256-rule fixture while other validation jobs were active. Raw observations are
in `target/issue3-performance-final.log` and
`target/issue3-management-final.log`; cache state and host load affect timings.
