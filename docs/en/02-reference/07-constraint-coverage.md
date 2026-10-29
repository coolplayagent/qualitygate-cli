# Constraint coverage

[简体中文](../../zh/02-reference/07-constraint-coverage.md) · [Volume index](README.md)

Capability `constraints.coverage.v1` adds optional `constraints` to the policy.
It records source-bound clauses independently of the existing project-rule v1
DSL. Obtain sources through `rules source` and export each record's contract
with `qualitygate schema constraint`. The complete authoring and adoption
workflow is in the [packaged reference](../../../skills/qualitygate-cli/references/constraint-coverage.md).

## Clause records and adoption

Each clause declares its identity/version, exact section/hash, interpretation,
requiredness, realization and review examples. `checks` realizations reference
explicitly configured policy checks; `unsupported` realizations retain a reason
and evidence owner. Mechanism labels classify the five rule capabilities; they
do not implement missing operators or Agent host interception. Example
references are review inputs, not execution claims. Scope is inherited from
the mapped check definitions and the report's selection.

Read-only `config --show --format json` returns `constraint_bindings`, binding
the clause and effective mapped check definitions. Only authorized adoption adds
the matching review record. Copying a digest is not independent approval;
`local_candidate`, `caller_supplied_ref` and signed active policy retain their
existing authority meanings. Definition changes make the adoption stale.

## Execution status

Every nonempty inventory adds a final reserved `constraint-coverage` check.
The report retains each clause and the snapshot, policy, rules and task digests.
Required unsupported/unadopted/stale/missing execution evidence yields
`incomplete`; complete mapped violations yield `fail`. All mapped checks are
considered before deciding. Optional gaps remain visible, and a required clause
cannot silently become advisory because a mapped check has warning severity.
Verified clauses inherit the exact check scope, including delivery/path limits.

Normative sources cannot be excluded and are included in frozen policy
packages. Worktree changes do not affect staged checks; changed policy or source
bytes cannot reuse evidence from another snapshot. A missing source, ambiguous
section or changed section hash stays incomplete. Quick checks may leave mapped
commands pending; use the full final check for delivery.

## Limits and interpretation

Limits: 256 clauses, 64 checks per clause, 16 example references per polarity,
and the existing 1 MiB policy budget. Unknown fields, duplicates, empty mappings,
unknown policy checks, self-reference and malformed sources are rejected.
Coverage describes the declared inventory, not proof of exhaustive prose
extraction or sufficiency of its interpretation. The [bounded operator protocol](02-rules.md)
is available separately. [Agent contracts](09-agent-contracts.md) and the
[Rust host](10-agent-host-and-delivery.md) provide their own capability-gated interfaces.


## Inspect coverage

```bash
qualitygate capabilities --format json
qualitygate schema constraint --format json
qualitygate config --show --format json
qualitygate check --worktree --profile full --format json
```

Require `constraints.coverage.v1`; the package version alone does not establish
support. Review `constraint_bindings` before adoption, then inspect the automatic
check's `metadata.constraint_coverage` in the report.

| Clause status | Meaning |
| --- | --- |
| `unsupported` | No supported evidence mechanism is mapped. |
| `unadopted`, `stale_adoption`, `source_invalid` | Review or source binding is missing or no longer valid. |
| `pending` | A mapped check is missing, unselected, disabled or incomplete. |
| `failed` | Complete mapped evidence contains a violation. |
| `verified` | Mapped checks passed for the report's subject and scope. |
| `not_applicable` | The mapped checks explicitly reported non-applicability. |

---

[Previous: Runtime capabilities and environment preflight](06-runtime-and-preflight.md) · [Contents](../README.md) · [Next: Consuming-agent contracts](09-agent-contracts.md)
