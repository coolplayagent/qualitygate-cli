# Fixture regression corpus

The compiled `qualitygate selfcheck` command reads the cases in `minimal`,
`typical` and `stress`, and compares production evaluator output with the
independently authored assertions in `golden`. Fixtures are shipped inside the
binary, including Cargo and Bazel builds; running selfcheck needs no checkout
or candidate policy. Rule definitions still come from the active Skill assets.

Each built-in rule must have both compliant and violating cases in every
suite. Catalog growth without those pairs makes selfcheck incomplete. The
corpus also exercises all ten external report formats, the custom file DSL,
strict policy loading, gate decisions, native Git snapshots and process capture.

The 416-case corpus includes 85 policy-evolution fixtures. Separate issue 9
case files and goldens add compliant and violating cases for four rules to
each suite. Separate issue 10 pairs do the same for three C/C++ and test-file
rules. Separate issue 11 pairs cover six Python and language-neutral rules. Run
`qualitygate selfcheck --rule policy-evolution --format json` for paired oracles,
category context, protected suite budgets, synthetic signatures, immutable
archives, lifecycle changes, and nine real paired Git workflows. Native cases
use disposable repositories/trust roots and fixed producers; they never sign
or promote a policy in the caller's repository.

See [selfcheck](../docs/selfcheck.md) for commands, diagnostics, budgets and
the distinction between falsification evidence and production assumptions.
