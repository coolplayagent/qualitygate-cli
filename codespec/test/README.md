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

`command_applicability` is a separate integration target for opt-in command
selection. Its isolated Git repositories distinguish non-applicable commands from
failed or incomplete execution and retain dependency and policy-change behavior.
Per-check syntax reuse has separate native adapter tests for cache bounds and
input identity; existing structure-rule regressions retain cross-file semantics.

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

`doctor` is a separate Rust integration target. Its `doctor/budgets.rs` and
`doctor/probes.rs` modules cover selected-snapshot budgets, native producer
execution boundaries and incomplete evidence. `doctor/native_probe.rs` is a
small Rust producer compiled into temporary storage by the test harness;
production executable identity limits remain unchanged. Windows PowerShell and
Git Bash invocations have a distinct native CI step and require its actual result.

## Resource, concurrency and work audit

The requirements in [§19](../requirements/qualitygate-cli.md#19-资源并发与工作量约束)
have separate semantic regressions and resource-shape evidence. The bilingual
[evidence index](../../docs/en/04-contributor-guide/03-requirements-to-test-evidence.md)
is the maintained cross-reference; these tests do not claim a universal runtime
or process-memory bound.

| Contract | Repeatable native evidence |
| --- | --- |
| GRAPH-WORK-01 | `config::dependencies::tests`: stable first-eligible policy order, invalid graph rejection, 10,000-node reverse-chain closure and profile/pending/disabled behavior; command applicability regressions retain prerequisite semantics. |
| SCOPE-INDEX-01 | `snapshot::scope_index::tests`: indexed versus single-call answers for current/old paths, duplicate aliases, changed lines and feedback scopes; rebuilding after public mutation; 2,000 changes queried against 5,000 unchanged paths. |
| BUILTIN-WORK-01 | `adapters::rules::builtin_limits_tests`: a small source that expands beyond 8 MiB, 10,001 structural findings, shared reservations across 32 parallel files and multiline range intersections; `adapters::diagnostic_budget::tests::builtin_workers_reject_expired_deadlines_before_retaining_findings`; marker adjacency and 2,000-entity binding tests. |
| FACT-WORK-01 | `adapters::custom_rules::selection::tests::every_rule_version_bounds_repeated_marker_declaration_bytes` checks all three schema versions; `selectors_without_an_external_budget_still_reject_excess_subjects` rejects excessive local collections. Existing named-source tests retain shared allowance and prior findings. |
| HISTORY-WORK-01 | `snapshot::messages::tests`: real isolated Git commits with order, alternate encoding, newline and NUL cases; 9 MiB aggregate rejection without partial success; parser framing/count/byte boundaries. `snapshot::history::tests`: injectable short deadlines for pending work and late completion, unique-content interning and pre-expansion accounting; `snapshot::git::tests::expired_git_parsing_stops_before_returning_partial_inputs`. |
| ASYNC-WORK-01 | `net::tests::maximum_provider_body_preserves_decoded_values_and_exact_wire_identity`: exact 1 MiB wire identity; `interfaces::render::tests::report_output_preserves_evidence_and_limits_severity_filter_to_diagnostics`; `application::feedback::tests::retaining_feedback_preserves_report_and_exact_artifact_identity`. |

The audit includes snapshot selectors, worktree/Git acquisition, history,
materialization/input guards, delivery archives, path confinement, declaration
and syntax processing, command scheduling, network decoding and report ownership.
Existing bounds remain relevant: four snapshot I/O workers, per-file/tree limits,
joined scoped workers, cancellation-held permits, regular-file and symlink checks,
and immutable input/delivery digests. The new tests exercise the changed paths;
the complete stable suite and architecture evidence cover integration boundaries.

Runner cancellation retains its Unix process-group / Windows Job boundary.
`runner::tests::timeout_kills_descendants_and_does_not_wait_for_inherited_pipes_forever`
and `cancelling_the_future_kills_descendants` exercise ordinary descendants.
They do not prove containment of a Unix descendant that starts another session
or group. Native Windows CI is separate evidence. Cooperative deadline checks
also cannot forcibly interrupt an OS filesystem call already in progress.

Performance observations must name the compared source revisions, fixture size,
command, build mode and measured quantity. Do not turn diagnostic benchmarks into
timing pass thresholds or infer production benefit from synthetic unit fixtures.
Stable checks, >=90% line coverage, Miri and ASan remain distinct required records;
this matrix states coverage, not an assertion that those runs have completed.

### Bounded before/after observations

The baseline is `bafce79` (`origin/main` before this audit); the comparison uses
this audit's working-tree implementation. Both use an unoptimized development
library built with `cargo build --lib` and standalone Rust harnesses linked by
`rustc --edition=2024 <source> -L dependency=target/debug/deps --extern qualitygate=<rlib> --extern serde_json=<rlib> -o <temporary-binary>`.
Each cell is one observation on the same environment, not a timing threshold or
a claim about real-repository latency. Times exclude compilation.

| Fixture / measured operation | Size | Before (ms) | After (ms) |
| --- | ---: | ---: | ---: |
| Reverse prerequisite chain / `Plan::build` | 500 checks | 598.207 | 10.712 |
| Reverse prerequisite chain / `Plan::build` | 1,000 checks | 2,364.111 | 18.604 |
| Reverse prerequisite chain / `Plan::build` | 2,000 checks | 11,180.592 | 42.008 |
| `comment-language` evaluation | 2,000 lines | 145 | 11.221 |
| `comment-language` evaluation | 4,000 lines | 560 | 21.292 |
| `comment-language` evaluation | 8,000 lines | 2,206 | 42.421 |

The plan fixture declares `c0..cN-1`, each check depending on the next, and the
final check has no prerequisites. All use argv `["git", "--version"]`; the full-profile
plan is measured without executing commands. The comment fixture is `src/a.rs`
containing repeated `// comment\n`, all lines added, with `language: english`.

A separate `source-pattern` amplification fixture contains 9,000 `x\n` lines
and one Rust prohibited pattern formed by repeating `a?` 256 times. Previously
it completed with 9,000 records and about 12.9 MB of diagnostic JSON. The bounded
implementation returns blocked with no verdict, 5,839 retained records and
8,388,430 serialized bytes (within 8 MiB). Check identifiers differ between the
two harnesses, so the old byte size is intentionally approximate. This fixture
demonstrates failure behavior at the cap, not a finding-count compatibility
promise once evidence is incomplete.
