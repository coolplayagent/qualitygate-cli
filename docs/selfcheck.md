# Fixture regression and verification boundaries

`qualitygate selfcheck` checks the installed implementation against the bundled
falsification corpus. It can expose a regression on a known shape; agreement
with the goldens cannot prove correctness on every real repository or deployment.

```bash
qualitygate selfcheck --format json
qualitygate selfcheck --fixture minimal
qualitygate selfcheck --rule commit-message
qualitygate selfcheck --fixture stress --rule commit-message --format markdown
```

The command works without a Git repository or `qualitygate.yaml`. It does not
read candidate policy or run candidate project commands. It evaluates the
active installed rule definitions, including configured defaults, against
compiled fixture inputs. Scenario-specific parameters are explicit in the
corpus. Git and the running CLI executable are the only subprocesses used by
native fixtures, inside disposable directories. No user Git configuration is
changed. The fixed hidden child probe exercises success, failure, a three-second
delay and bounded output overflow; it accepts no arbitrary command.

Native snapshot cases reject inherited `GIT_DIR`, `GIT_INDEX_FILE`, object-store
and related Git overrides before any temporary repository operation. Such an
environment is incomplete evidence and must be rerun without those overrides;
selfcheck never lets them redirect writes to an existing repository.

## Meaning of a result

| Exit | Meaning |
|---|---|
| 0 | Every selected fixture agrees with its golden assertions. A deliberately violating input must produce the expected failing verdict. |
| 1 | At least one observed result disagrees with its golden. |
| 2 | Selfcheck could not complete, including missing assets/tools, corrupt inventory, a worker failure, budget exhaustion or an empty selection. |

An expected timeout or incomplete rule input can agree with a golden. A fixture
setup failure cannot substitute for the expected rejection: it is separately
reported as an incomplete fixture. Unknown rule filters and missing
compliant/violating coverage never result in an empty success.

JSON includes suite/fixture IDs, an input file plus JSON Pointer, input and
golden SHA-256 digests, corpus and active-rule digests, observed output, and
each mismatching assertion's expected and actual values. Table and Markdown
retain the same decision and diagnostic identity. The corpus source is under
[fixtures](../fixtures/README.md); compiled assets make release selfchecks
independent of source paths on the user's machine. A corpus change requires
rebuilding the binary.

## Coverage and budgets

All fourteen built-in rule IDs have compliant/violating goldens in each suite.
The corpus also covers the ten normalized report formats, the custom file DSL,
policy parsing, gate completeness, native snapshots and process capture.
Signed synthetic review approval/rejection and japicmp compatibility inventory
fixtures cover those checker boundaries without external evidence or producers.
Minimal cases isolate one behavior. Typical cases retain existing code and
exercise common framework/source forms. Stress cases cover malformed inputs,
Unicode, physical long paths, symlink modes, missing objects, file/output
limits, timeouts and changed/foreign evidence.

Each suite is limited to 512 cases and 2 MiB of input plus golden text. Cases
run sequentially on a blocking worker, keeping parsing/filesystem setup off
async orchestration threads. Scheduling stops after 60 seconds; native snapshot
fixtures have a ten-second deadline, native commands have at most ten seconds,
and rule parsers retain their production budgets. One in-progress bounded
evaluator can finish after the scheduling deadline. Snapshot files retain the
2 MiB limit, and captured process streams retain the 16 MiB limit.

Synthetic analyzer and project facts test normalization and rule decisions;
they do not execute those producers. Existing live-producer integration gates,
Miri for the pure domain, and native ASan tests remain separate. A successful
Linux run is not Windows evidence. Unrepresented framework semantics,
configuration delivery, runtime reflection, filesystem behavior and production
tool versions remain explicit assumptions.

## Check report conclusions

Ordinary `check` reports and `selfcheck` reports carry `verification` with
`conclusion`, `verified_shapes`, `known_limits` and `unverified_assumptions`.
A clean conclusion says “在已验证形态下未发现问题” (no problems found in the
verified shapes). A satisfied blocking gate with warning findings explicitly
retains those findings for review. Failing/incomplete reports cannot use the
clean conclusion. Existing machine verdicts and exit codes remain unchanged.
For ordinary checks, verified shapes identify completed checks and matched
entities in the selected snapshot; they do not claim that selfcheck ran.

## Regression verification

`tests/selfcheck.rs` runs the full installed command surface, filters, native
boundaries and all report formats. Its mutation test copies the rule assets
into a temporary directory, changes the commit pattern to accept empty text,
and requires selfcheck to fail on the unchanged `commit-empty` golden. This
tests the falsification mechanism, not just a precomputed success report.

The PR workflow runs minimal selfcheck as its own gate. The scheduled
cross-platform workflow runs the full corpus and uploads JSON even on failure.
Local full repository policy includes selfcheck alongside the existing gates.
