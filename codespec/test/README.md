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
