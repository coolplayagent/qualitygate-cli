# Test effectiveness on paired snapshots

An opt-in command check can require each added or content-modified independent
test file to expose an assertion failure on the old code, after passing on the
current code. This measures the selected tests against one concrete historical
implementation. It does not establish that every assertion is useful or that
all possible defects are detected.

## Configuration

Use the existing `check` command and snapshot selectors. There is no automatic
activation, runner detection, title-based exemption or policy approval.

```yaml
schema_version: 1
checks:
  - id: behavior-counterexamples
    argv: [project-test-runner, --junit, target/tests.xml]
    timeout_seconds: 300
    findings_exit_codes: [1]
    tools:
      - id: project-test-runner
        argv: [project-test-runner, --version]
    reports:
      - path: target/tests.xml
        format: junit
        minimum_tests: 1
    test_effectiveness:
      source_paths: ['src/**']
      test_paths: ['tests/**']
      support_paths: ['test-data/**']
      assertion_failure_types: [AssertionError]
```

Replace the producer, paths and exact assertion types with reviewed project
facts. Each path group accepts at most 256 unique repository-relative globs;
source and test groups must be nonempty. Failure types require 1..256 distinct
nonblank exact names, at most 256 bytes each. Unknown fields are rejected.
The same nested configuration is available in task acceptance `verification`.

The command requires tool probes, success exit code zero, explicit findings
exit codes and exactly one `full` JUnit report. Report baselines, coverage,
project-output mappings and compatibility checks cannot be combined with it.
Declare repository-owned runner assets in `tools[].inputs`. These assets must
remain comparable in both executions.

## Selection and composition

The comparison's already resolved base is the old-code version. No extra merge
base is selected. Source paths must match captured files within the requested
scope. An empty source inventory is incomplete. If none of those files changed,
the check is not applicable and retains that reason. The existing overall gate
still rejects an empty plan or a plan containing only skipped checks.

Production changes without an added or content-modified test file are a
violation. A pure test rename, deletion or executable-bit change does not create
a new counterexample obligation. Scope filters select obligations; scoped
checks cannot establish delivery readiness.

Current tests run first in an isolated materialization. If they pass, a second
materialization contains the old snapshot plus current files matched by the
test and support selectors. This includes additions, replacements, removals
and file modes. All other files remain from the old snapshot. Production paths
must not overlap test/support paths. Changed policy assets and recognized build
manifests or lockfiles cannot be transplanted; add project-specific build inputs
to `verification_assets`. The overlay preserves deletions and does not copy the
user's working directory or installed dependencies into the baseline.

Both executions use identical argv and cwd with fresh reports, tool identity
checks and input guards. Staged selection uses index bytes even if the worktree
contains another repair. Policy selection, active-policy verification and final
source revalidation retain their existing semantics. Rust inline tests are not
extracted; use independent test files and a producer satisfying the report
profile. If new dependencies or APIs prevent the old code from compiling or
running, the result remains incomplete.

## Required JUnit profile

Each `testcase` needs a nonblank `name` and an explicit `file` resolving to a
captured input. Absolute paths must lie within that execution's materialization.
The tuple `(file, classname, name)` is the identity; an omitted class is empty.
Identities must be unique, and selected case inventories must match across runs.
The CLI does not infer file ownership from class names or console output.

A `<failure>` counts only when every associated `type` is in the configured
allowlist. `<error>`, unknown or absent failure types, contradictory counters
or outcomes, nested cases and outcomes outside a testcase are incomplete.
Selected skipped cases, missing selected files and insufficient actual test
counts are also incomplete. Raw reports and producer identity are retained;
failure classifications remain claims made by that producer. A generic JUnit
export lacking these fields needs a suitable producer configuration or adapter.
Ordinary command checks retain their existing JUnit behavior.

| Observation | Check result |
|---|---|
| Current tests pass; every changed test file has an identical case with an allowed baseline assertion failure | Pass |
| Current tests have known assertion failures | Violation; baseline is not run |
| Baseline completes but a changed test file has no assertion counterexample | Violation for that file |
| Missing/invalid reports, compilation errors, unknown failures, changed case identities, timeout or invalid inputs | Incomplete |

Baseline failures must agree with a declared findings exit code. A zero exit
alongside baseline assertion failures is inconsistent evidence. Unrelated test
failures never satisfy another file's obligation. `required`, `severity`, exit
codes 0/1/2, warning visibility and pending delivery remain unchanged.

## Evidence and resource limits

JSON retains `test_effectiveness` selection, overlay content digests, current
and composed snapshot identities; `current_test_execution` and
`baseline_test_execution` retain each actual execution and its normalized cases.
`test_effectiveness_files` records executed tests and counterexamples per file.
Table and Markdown show the same per-file counts and violations. Diagnostics
keep stable file-specific identities and the existing pinned recheck command.

The two runs execute sequentially. They share one `timeout_seconds` deadline
starting before current tool probes; remaining time bounds each probe and main
process, with partial logs retained. Existing bounded materialization, input
verification and report collection operate around these executions. Composition
has a 30-second deadline and respects the caller's snapshot byte budget,
100,000-file and 2 MiB-per-file limits. JUnit parsing accepts at most 2 MiB,
100,000 XML nodes and 50,000 cases. Process streams retain the 16 MiB limits.

## Requirement-to-test evidence

| Requirement | Regression evidence |
|---|---|
| Per-file counterexamples and pure incomplete/violation separation | `domain::test_effectiveness::tests` |
| Strict policy, task/reusable-core configuration and ordinary compatibility | `config::test_effectiveness::tests`, `adapters::reports::test_cases::tests` |
| Immutable composition, deletions, renames, modes and resource bounds | `snapshot::test_overlay::tests` |
| Actual assertions, multi-file proof and retained artifact digests | `tests/test_effectiveness.rs::real_assertions_prove_each_changed_file_and_retain_both_executions` |
| Missing evidence, errors, skips, timeout, mutation and changed tools | `execution_gaps_and_non_assertion_failures_never_establish_effectiveness` |
| Staged/path selection, support files, selected policy and tasks | `snapshot_selection_support_overlay_and_policy_authority_are_preserved` |
| Empty inventories, overlapping selectors, protected assets and output collisions | `missing_scopes_protected_inputs_and_report_collisions_fail_closed` |

The integration producer executes Rust assertions in temporary repositories.
These controlled fixtures do not certify an external runner's classification
or replace the real-repository pilot. Domain tests participate in Miri; native
unit tests remain in the separate ASan gate.
