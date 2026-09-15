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
