# Test

This directory is governed by `codespec/codespec-map.yaml`. Update its map entry through `relay-knowledge map directory` and keep reviewed source material within the declared content scope.

The [acceptance evidence index](../../docs/acceptance-evidence.md) separates
unit, integration, architecture, documentation, coverage, Miri and ASan gates.
The [evolution matrix](../../docs/policy-evolution.md) maps policy behavior to
tests; [performance fixtures](../../docs/large-repositories.md) establish
serial/parallel agreement and report observed costs under explicit budgets.
The [bundled corpus](../../docs/selfcheck.md) contains 85 policy-evolution
fixtures with independently authored golden assertions. Its native Git,
signature, lifecycle and failure cases run in the shipped CLI; a mutation test
must detect a weakened evaluator without editing either acceptance or goldens.

`file_contracts` and `ratchet` are distinct integration targets for the new
file/report assertions. Pure count decisions remain in the native domain unit
target and Miri selection; adapter bounds remain in native unit tests.

The [Skill release contract](../../docs/skill-package.md#verification) checks
that published Markdown links and the v7 pilot template resolve inside the
Skill package, in addition to CLI/schema and release-archive checks.

`test_effectiveness` is a separate Rust integration target with actual assertion
execution in temporary repositories; [its evidence matrix](../../docs/test-effectiveness.md#requirement-to-test-evidence)
links the pure decision, JUnit profile, immutable composition and failure cases.
The four `custom-contract-*` selfcheck fixtures cover required text and nonempty
entity matching, including malformed syntax as incomplete evidence.

The [phase-A evidence matrix](../../docs/pilot-phase-a.md#5-需求到测试及阶段验收)
maps versioned bug-fix/refactor templates to `pilot_baseline` integration tests.
`pilot_codex_live` separates ordinary protocol-parser tests, explicitly invoked
model probes and offline reassessment of retained artifacts. These engineering
records remain separate from real-task repair and benefit acceptance.

The [phase-B matrix](../../docs/pilot-phase-b.md#3-需求到测试) maps pure feedback
projection tests, `feedback` and `agent_loop` integration targets, the merged-branch
regression, and the separately invoked `pilot_repair_live` model experiment.

The [phase-C matrix](../../docs/pilot-phase-c.md#4-任务模板与需求到测试) maps
`case_provenance`, pure pilot metric tests, `pilot_summary` report bindings and
`pilot_templates` producer checks. The offline replay example consumes retained
artifacts without invoking an Agent or claiming business-trial acceptance.

The [phase-D matrix](../../docs/pilot-phase-d.md#4-需求到测试) maps pure plan-seal
invariants and the `pilot_seal` CLI target. It covers late sealing, incomplete
governance/matrices, permitted actual-model observations and protected plan
mutation without treating a digest as a trusted signature.

The [phase-E matrix](../../docs/pilot-phase-e.md#5-需求到测试) separates adapter
signature/validity tests from the `pilot_authorization` CLI target. It covers
exact-subject binding, owner scope, external inputs, expiry, revocation and the
explicit absent state without treating start authorization as final acceptance.

The [phase-F matrix](../../docs/pilot-phase-f.md#6-需求到测试) separates pure
threshold decisions, final-review signature authentication and full CLI evidence
binding. It covers accepted, rejected, failed, unknown, revoked and stale evidence.

The [phase-G matrix](../../docs/pilot-phase-g.md#3-需求到测试) covers v2 budget
sealing, all-attempt and human-time accounting, unknown versus exceeded cost,
and the signed tenth threshold while preserving v1 fixtures.

The [phase-H matrix](../../docs/pilot-phase-h.md#3-需求到测试) covers v3 task mix,
distinct-input roster, duplicate task identity, seal drift and v1/v2 compatibility.

The [phase-I matrix](../../docs/pilot-phase-i.md#3-需求到测试) covers v4 source
identity, bounded artifact rereads, path/digest failures, plan drift and v3
compatibility.

The [phase-J matrix](../../docs/pilot-phase-j.md#3-需求到测试) covers v5 run-order
permutation, workflow alternation, stratum balance, observed deviations and v4
compatibility.

The [phase-K matrix](../../docs/pilot-phase-k.md#3-需求到测试) covers v6 baseline
binding, strict progress, budget and stop-rule deviations, archive failures and
v5 compatibility.

The [phase-L matrix](../../docs/pilot-phase-l.md#需求到测试) covers v7 model
capture binding, explicit unknown, missing or changed archives, path bounds and
v6 compatibility.
