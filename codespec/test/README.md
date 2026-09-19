# Test

This directory is governed by `codespec/codespec-map.yaml`. Update its map entry through `relay-knowledge map directory` and keep reviewed source material within the declared content scope.

The [acceptance evidence index](../../docs/en/04-contributor-guide/03-requirements-to-test-evidence.md) separates
unit, integration, architecture, documentation, coverage, Miri and ASan gates.
The [evolution matrix](../../docs/en/02-reference/04-policy-task-and-signed-evidence.md) maps policy behavior to
tests; [performance fixtures](../../docs/en/01-user-guide/02-snapshots-and-check-workflow.md) establish
serial/parallel agreement and report observed costs under explicit budgets.
The [bundled corpus](../../docs/en/04-contributor-guide/02-selfcheck-and-fixtures.md) contains 85 policy-evolution
fixtures with independently authored golden assertions. Its native Git,
signature, lifecycle and failure cases run in the shipped CLI; a mutation test
must detect a weakened evaluator without editing either acceptance or goldens.

`file_contracts` and `ratchet` are distinct integration targets for the new
file/report assertions. Pure count decisions remain in the native domain unit
target and Miri selection; adapter bounds remain in native unit tests.

The [Skill release contract](../../docs/en/03-architecture/04-physical-view.md) checks
that published Markdown links and the v7 pilot template resolve inside the
Skill package, in addition to CLI/schema and release-archive checks.

`test_effectiveness` is a separate Rust integration target with actual assertion
execution in temporary repositories; [its evidence matrix](../../docs/en/02-reference/02-rules.md)
links the pure decision, JUnit profile, immutable composition and failure cases.
The four `custom-contract-*` selfcheck fixtures cover required text and nonempty
entity matching, including malformed syntax as incomplete evidence.

The [phase-A evidence matrix](../../docs/en/01-user-guide/05-real-repository-pilot.md)
maps versioned bug-fix/refactor templates to `pilot_baseline` integration tests.
`pilot_codex_live` separates ordinary protocol-parser tests, explicitly invoked
model probes and offline reassessment of retained artifacts. These engineering
records remain separate from real-task repair and benefit acceptance.

The [phase-B matrix](../../docs/en/01-user-guide/05-real-repository-pilot.md) maps pure feedback
projection tests, `feedback` and `agent_loop` integration targets, the merged-branch
regression, and the separately invoked `pilot_repair_live` model experiment.

The [phase-C matrix](../../docs/en/01-user-guide/05-real-repository-pilot.md) maps
`case_provenance`, pure pilot metric tests, `pilot_summary` report bindings and
`pilot_templates` producer checks. The offline replay example consumes retained
artifacts without invoking an Agent or claiming business-trial acceptance.

The [phase-D matrix](../../docs/en/01-user-guide/05-real-repository-pilot.md) maps pure plan-seal
invariants and the `pilot_seal` CLI target. It covers late sealing, incomplete
governance/matrices, permitted actual-model observations and protected plan
mutation without treating a digest as a trusted signature.

The [phase-E matrix](../../docs/en/01-user-guide/05-real-repository-pilot.md) separates adapter
signature/validity tests from the `pilot_authorization` CLI target. It covers
exact-subject binding, owner scope, external inputs, expiry, revocation and the
explicit absent state without treating start authorization as final acceptance.

The [phase-F matrix](../../docs/en/01-user-guide/05-real-repository-pilot.md) separates pure
threshold decisions, final-review signature authentication and full CLI evidence
binding. It covers accepted, rejected, failed, unknown, revoked and stale evidence.

The [phase-G matrix](../../docs/en/01-user-guide/05-real-repository-pilot.md) covers v2 budget
sealing, all-attempt and human-time accounting, unknown versus exceeded cost,
and the signed tenth threshold while preserving v1 fixtures.

The [phase-H matrix](../../docs/en/01-user-guide/05-real-repository-pilot.md) covers v3 task mix,
distinct-input roster, duplicate task identity, seal drift and v1/v2 compatibility.

The [phase-I matrix](../../docs/en/01-user-guide/05-real-repository-pilot.md) covers v4 source
identity, bounded artifact rereads, path/digest failures, plan drift and v3
compatibility.

The [phase-J matrix](../../docs/en/01-user-guide/05-real-repository-pilot.md) covers v5 run-order
permutation, workflow alternation, stratum balance, observed deviations and v4
compatibility.

The [phase-K matrix](../../docs/en/01-user-guide/05-real-repository-pilot.md) covers v6 baseline
binding, strict progress, budget and stop-rule deviations, archive failures and
v5 compatibility.

The [phase-L matrix](../../docs/en/01-user-guide/05-real-repository-pilot.md) covers v7 model
capture binding, explicit unknown, missing or changed archives, path bounds and
v6 compatibility.
