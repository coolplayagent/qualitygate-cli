# Fixture regression and verification boundaries

`qualitygate selfcheck` checks the installed implementation against the bundled
falsification corpus. It can expose a regression on a known shape; agreement
with the goldens cannot prove correctness on every real repository or deployment.

```bash
qualitygate selfcheck --format json
qualitygate selfcheck --fixture minimal
qualitygate selfcheck --rule commit-message
qualitygate selfcheck --rule policy-evolution --format json
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
The policy timeout fixture uses a fixed Git shell alias to launch that same
probe with a one-second task deadline. The alias quotes the running executable
path and accepts no caller-supplied command. Other policy task probes use
`git --version`; the missing-tool case names an absent temporary file. Debug
evaluator size and missing producers do not change production identity limits.

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

The corpus contains **338 fixtures**, including **85 policy-evolution fixtures**.
`--rule policy-evolution` selects this regression group; it is not an installable
rule ID.

| Shapes | Count | Evidence |
|---|---:|---|
| Category context and strict membership | 7 | Independent category filters, mandatory rules/checks under an empty filter, scalar compatibility, invalid memberships |
| Protected suite validation | 11 | Valid suite, memory/snapshot/concurrency and contribution bounds, distinct snapshots/tasks, full commit IDs, independent anchors/oracles, strict fields |
| Paired oracle and effectiveness | 18 | Intentional negative gates, held-out/anchor regressions, absent/skipped checks, missing cases/snapshots, timeout precedence, pending delivery, changed inputs/producers, activation/cost/benefit separation |
| Approval and rollback signatures | 26 | Independent human principals, rejection, self-approval, wrong identity/subject/type/key, tampering, revocation, expiry/future/excessive lifetime |
| Native candidate archives/lifecycle | 14 | Immutable parent/revisions/authorship, retained evidence/history, rejection freeze, missing/foreign/corrupt records, unapproved promotion, five lifecycle states and actor/evidence binding |
| Native paired Git workflows | 9 | Serial/parallel promotion, oracle block, actual missing-tool/timeout states, contribution block, wrong baseline, signed rollback and stale history |

Inputs and goldens are authored separately. Fixed native setup is reviewable
in the Rust [policy harness](../src/qualitygate/application/selfcheck_policy.rs)
and [workflow harness](../src/qualitygate/application/selfcheck_policy_io.rs).
Setup errors remain fixture execution gaps. An evaluation that disagrees with
its golden is a regression; dependent promotion is attempted only after a
passing evaluation. Missing-tool and timeout goldens assert the actual producer
execution status, preventing unrelated incomplete results from satisfying them.
The contribution fixture adds a new rule definition; enabling an existing
definition does not grow the library.

Promotion fixtures remove the original execution directory before promotion,
requiring retained artifacts to support approval. All archives, trust roots
and public deterministic fixture-key signatures use temporary directories
and are removed afterward. Report references describe disposable fixture
records, not externally queryable production approvals.

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
An additional mutation substitutes the line-ending evaluator with the real
diff-size evaluator. The unchanged policy acceptance oracle must block the CRLF
replay and the unchanged promotion golden must make selfcheck fail, with no
active policy. Integration assertions require identical content digests and
paired oracle outcomes for serial/parallel native fixtures and reject inherited
Git redirection before native workflow setup.

The PR workflow runs minimal selfcheck as its own gate. The scheduled
cross-platform workflow runs the full corpus and uploads JSON even on failure.
Local full repository policy includes selfcheck alongside the existing gates.

The 2026-09-14 Linux fixture expansion passed all 334 fixtures and nine selfcheck
integration tests, including 85 new cases with 335 independent assertions. The
complete repository run passed 335 tests and 95.44% Rust line coverage; Miri
passed 19 pure-domain tests and ASan passed 185 native unit tests. The
ASan CLI additionally completed all 334 fixtures with leak detection and no
sanitizer diagnostics. The measured
results and a separately retained under-load budget exhaustion are recorded in
[policy evolution verification](policy-evolution.md#current-implementation-verification).
