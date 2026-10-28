# Exit codes and resource limits

[简体中文](../../zh/02-reference/05-exit-codes-and-resource-limits.md) · [Volume index](README.md)

## Exit codes

| Code | Meaning | Required interpretation |
| --- | --- | --- |
| `0` | Complete pass for the selected scope | Retain scope, snapshot, policy, warnings, and evidence limits. |
| `1` | Complete execution with a blocking violation | Repair or obtain an authorized policy decision; do not weaken checks silently. |
| `2` | Incomplete validation | Supply missing prerequisites or evidence and rerun; never report pass. |

Completion and decision are independent. A completed check can fail because it
found a violation. A process failure, timeout, missing report, unsupported
capability, stale input, or unexecuted required check produces incomplete
validation. Warning filters affect display, not the computed gate.

## Default snapshot bounds

YAML policy, task, project-rule, validation-suite, packaged-rule and external
policy-trust inputs are limited to 1 MiB before parsing. A shared preflight
limits expansion to 100,000 visited values (including mapping keys and tag
names), 4 MiB of UTF-8 string content and 64 levels beneath the root. Every
alias visit is charged again, before constructing YAML Values or typed
collections; quoted alias-like text is ordinary content. These are deterministic
input budgets, not a process-memory or elapsed-time guarantee. Project rules
retain their additional 50,000-node shape limit. Oversized configuration or task
inputs produce incomplete validation; `rules validate` reports an invalid YAML
rule and generation refuses it. Inputs are preserved for repair and rechecking.

Before scalar type resolution, an event preflight also limits expanded scalar
text to 16 MiB, including numeric spelling, mapping keys and resolved tag text.
Each alias charges its referenced subtree's text again; anchor costs are cached
without expanding collections or repeatedly parsing numeric values. UTF-8 and
its optional BOM retain the existing decoder's encoding contract. This work
budget supplements the string, node and depth bounds above; it is not a wall
clock deadline. Budget diagnostics retain the offending event's line and column.

The normal acquisition envelope supports up to 100,000 files, 256 MiB total
contents per tree, 2 MiB per file, four content readers, and 120 seconds. Git
objects normally use batches no larger than 4 MiB and 1,024 objects; explicitly
permitted larger files use singleton batches up to 8 MiB, below the unchanged
16 MiB process output limit. Caller
controls are:

- `--snapshot-max-mib` from 1 to 1024;
- `--snapshot-max-file-mib` from 1 to 8 (default 2), independent of the total budget;
- `--snapshot-jobs` from 1 to 16;
- `--snapshot-timeout-secs` from 1 to 3600.

Process capture is bounded to 16 MiB per stream. Individual adapters and rules
also bound report bytes, selected files, source lines, diagnostics, parser work,
subprocess time, and concurrency. The effective report records which limit was
hit. Increasing a caller budget does not authorize broader paths or policy.

Executable identity accepts regular files, including symlinks resolving to a
regular file. It checks file type and size before opening, then rechecks the
opened handle before hashing. Existing FIFOs, sockets and directories cannot
supply executable evidence. Producer binaries are limited to 256 MiB; the
Qualitygate evaluator allows 1 GiB for development debug sections. Hashing
checks a 30-second budget between reads. This is not protection against hostile
filesystem replacement races or a guarantee that OS filesystem calls terminate.

Configuration loading and repository discovery also reject special files before
opening and recheck the opened handle. Configuration reads are bounded to 1 MiB,
including growth during reading; discovery retains its 2 MiB per-input and
12 MiB aggregate limits. A FIFO configuration or root/nested `.gitignore`
returns incomplete instead of hanging or publishing an initialization candidate.
An existing unreadable input is not treated as a missing policy.

XML reports, Maven/project models, compatibility evidence and Maven discovery
limit element depth to 32, tree nodes to 100,000, attributes per element to 1,024,
and attributes per document to 100,000. Root elements, document root nodes and
namespace declarations count toward their respective limits. Iterative token
preflight checks depth and attributes before recursive tree parsing and duplicate
attribute comparisons. Attribute-like text in comments, CDATA, processing
instructions or attribute values does not consume attribute counts. Caller byte
limits and format-specific DTD handling still apply. Oversized execution evidence
is incomplete and retains collected raw artifacts; advisory discovery records a
manifest gap without rewriting an existing policy.

Namespace scope expansion and name lookup share 10,000,000 work units per
document. For each element, let `P` count ancestor declarations (including
shadowed prefixes), `D` count its declarations, `S = P + D`, and `L` count its
element name plus prefixed attribute names. Preflight charges `S * L`, plus
`P * S` when `D > 0`. Closing an element restores the parent scope without
refunding work. This conservative accounting bounds namespace amplification;
it is not a wall-clock prediction or a replacement for the other XML limits.

SARIF location resolution shares 200,000 reference steps and 16 MiB of visited
text across all runs. Text includes logical names, source URIs, URI base IDs and
resolved URIs; repeated references count again even when their locations later
deduplicate. Reference depth remains limited to 32 and each result to 256 primary
locations. Logical references borrow source strings until materialization.
Retained locations and normalized results each have a cumulative 16 MiB compact
JSON limit, including escaping. Locations are charged before append; results are
serialized into a bounded counter instead of a temporary JSON allocation.
Non-violation results also consume these budgets. Excessive expansion produces
incomplete evidence and retains raw report artifacts for repair and rechecking;
the budgets do not guarantee process memory or elapsed time.

Cargo Clippy, ESLint, golangci-lint, Ruff, JUnit, Checkstyle, PMD and SpotBugs
normalization also limits the cumulative compact JSON diagnostic records to
16 MiB per report, including string escaping. All files share the allowance;
each record is measured before appending it to the retained diagnostics. The
counter allocates no JSON copy. This bounds amplification from repeated file
paths before source mapping or delivery filtering. Excess reports remain
incomplete, with raw artifacts retained for repair and rechecking. The input
byte limits and final diagnostic-output limits still apply separately.

Report paths first try an exact captured path. A shortened path must uniquely
match a directory-boundary suffix across the captured inventory, including
files outside the selected delivery or coverage scope. Each report application
builds current and baseline suffix indexes only when needed, retaining one
borrowed path reference per file and sharing resolution across its diagnostics
and coverage. Path text and captured file contents are not copied into the index.

Built-in rule assets share a 256-file and 1 MiB limit across all packages. The
byte allowance is charged against actual reads, including growth or regular
files whose metadata understates their size. Each read is limited to the
remaining allowance plus one byte to detect overflow; excess assets make
catalog loading incomplete before YAML parsing. Exact limits are inclusive.
Asset symlinks, including Bazel runfiles, may resolve to regular files; type and
size are checked both before opening and on the opened handle.

## Diagnostic output

Each check retains at most 10,000 diagnostic records and 8 MiB of compact UTF-8
JSON in its diagnostics array, including escaped strings and replay arguments.
Project rules v1/v2/v3 share this allowance across file inventory, fixed
assertions, dependency findings and expressions. Records are charged before
being retained. The application checks the allowance again when binding replay
arguments, on its blocking worker pool.

Exhaustion leaves the affected check incomplete and preserves the retained
prefix of findings. `metadata.diagnostic_output` identifies the limit, phase
and retained count; it does not claim a complete count of omitted violations.
An earlier timeout or execution-gap reason is preserved. Narrow the check's
scope or shorten repeated repair instructions, then rerun the original scope
after resolving the findings. Existing policy authority and required-check
semantics still apply. Clause coverage uses the final bounded check results;
a diagnostic-output gap remains pending coverage.

## Failure boundaries

Path confinement rejects traversal and symlink escapes. Git conflicts,
submodules, special files, object mismatches, source mutation during execution,
and invalid UTF-8 where text is required fail closed. Network and environment
access are owned by explicit adapters; missing credentials or unavailable tools
are evidence gaps.

Keep unit, integration, live-producer, Miri, and ASan results separate. A test
fixture pass covers only its represented shapes. Real-repository, platform,
network, and human-review assumptions remain visible until independently
verified.

Doctor returns `0` only for the requested `scope: preflight`; blocked or incomplete
preflight returns `2`. It has no delivery verdict. See [runtime and preflight](06-runtime-and-preflight.md).

---

[Previous: Policy, task, and signed evidence](04-policy-task-and-signed-evidence.md) · [Contents](../README.md) · [Next: Runtime capabilities and environment preflight](06-runtime-and-preflight.md)
