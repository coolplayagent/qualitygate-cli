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
