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
