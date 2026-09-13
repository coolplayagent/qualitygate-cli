# Coverage producer profiles

Coverage gates evaluate fresh reports from a declared command on the selected Git snapshot. A valid measurement below the configured threshold is a violation (exit 1). Missing, contradictory or unmappable evidence makes the check incomplete (exit 2). [Report configuration](reports.md#coverage-inventory-and-thresholds) defines the required source inventory, full/changed-line scope and line/branch thresholds.

## Native coverage.py JSON

Use `format: coverage_py` for native JSON format 2 or 3. The producer must emit `meta.format`, a version, an explicit `meta.branch_coverage`, a nonempty `files` inventory and `totals`. File line inventories, branch arc identities and file/total summaries must agree. Duplicate file keys, repeated/conflicting line or branch records, missing counters and unknown format revisions are incomplete. Auxiliary function/class regions and contexts are retained in the raw report; the gate uses file-level facts.

```yaml
path: target/coverage.json
format: coverage_py
mode: changed_lines
coverage_paths: ['src/**/*.py']
minimum_coverage: 90
require_branch_coverage: true
```

The project's configured command must run coverage with branch collection, run its tests and generate JSON before exiting. Declare version probes for the actual Python, coverage and test tools, and use a JUnit report to establish executed test counts. The independent Rust live harness demonstrates this sequence with `coverage run --branch --source src -m pytest`, `coverage xml` and `coverage json` in one fresh workspace.

An explicit branch flag distinguishes measurement with zero branch arcs from disabled measurement. The former records a `null` branch percentage; the latter cannot satisfy a required branch contract. Negative arc destinations identify exits and are retained during identity validation. The adapter derives line and branch rates separately from counts, rather than using the producer's combined display percentage. These fields follow the [coverage.py JSON producer](https://github.com/nedbat/coveragepy/blob/7.10.7/coverage/jsonreport.py).

Excluded lines remain visible in normalized records and `excluded_lines` metadata and must fit the selected source. They do not contribute to the threshold denominator. Executed excluded statements do not become covered executable statements. Any branch-exemption output whose summary cannot be reconstructed from its reported arcs, including certain `no branch` suppressions, is incomplete; the adapter does not invent covered arcs. Source exclusions and coverage configuration require the same policy/input review as other verification assets.

The Python 3.12/coverage.py 7.10.7 live fixture exposes an empty-module sentinel: line-only measurement can emit `executed_lines: [0]` with zero statements. Only this isolated zero record with no missing/excluded statements and consistent zero summaries is treated as file inventory. Other zero-line records are rejected. The original JSON always remains available.

## Cobertura XML

The adapter uses class-level line records. Method-level copies do not add to the denominator. Root line and branch counters, when present, must match the normalized records. A true branch flag requires valid hit/total counts; percentages must agree within the producer's one-percent rounding interval. Covered branches cannot accompany an uncovered line. Duplicate source lines are incomplete.

`sources/source` entries resolve shortened filenames against the selected snapshot. Explicit roots must yield exactly one existing path after confinement; missing, foreign, traversing and ambiguous roots cannot fall back to suffix guesses. With no explicit roots, the existing unique report-path mapping applies. Every record is mapped and validated before scope filtering, including records outside the configured coverage paths.

Positive `branches-valid` establishes branch measurement. Zero does not distinguish a branch-free measured program from a tool invoked without branch measurement. A required branch contract therefore needs native JSON or another format that proves measurement. Setting `require_branch_coverage: false` is an explicit line-only policy choice. The [coverage.py XML producer](https://github.com/nedbat/coveragepy/blob/7.10.7/coverage/xmlreport.py) and [XML command documentation](https://coverage.readthedocs.io/en/7.10.7/commands/cmd_xml.html) describe its source-root and summary output.

## JaCoCo XML

Source lines carry instruction and branch counters. The adapter checks source-file instruction/line/branch totals and their package, group and report rollups. Missing positive counters, omitted source/debug evidence, repeated sources across groups, orphan lines, inconsistent summaries and count overflow are incomplete. Empty source files remain inventory entries. Source coordinates must fit the selected source snapshot.

The [JaCoCo report DTD](https://github.com/jacoco/jacoco/blob/v0.8.15/org.jacoco.report/src/org/jacoco/report/xml/report.dtd) declares the standard public identifier `-//JACOCO//DTD Report 1.1//EN` and system identifier `report.dtd`. The parser recognizes that exact pair as an inert header. It also recognizes Cobertura's standard `coverage-04.dtd` system identifier on the documented SourceForge HTTP/HTTPS and raw GitHub URLs. It never fetches or loads a DTD. Unknown declarations, internal subsets and entities remain rejected; this is report normalization, not general DTD validation.

## Bounds and retained evidence

Raw reports retain the existing 2 MiB byte limit. XML parsing permits at most 100,000 nodes; an accepted DTD declaration is at most 1 KiB. JaCoCo hierarchy depth is at most 32. Coverage paths are at most 16 KiB each and explicit source roots at most 64. Repeated filenames and mapped paths consume a conservative 16 MiB normalization budget before per-record expansion. LCOV normalization observes the same budget.

Reports use bounded asynchronous reads; parsing and coverage application use blocking workers. Original reports keep their byte digests, including accepted XML headers, and survive later parsing failures. Metadata records source scope, line/branch counts, exclusions, branch-measurement availability and native producer identity. Completed test failures, compiler/collection errors, deadlines and input mutations retain their existing distinct execution/completeness states.

## LCOV zero counters

The [LLVM LCOV exporter](https://github.com/llvm/llvm-project/blob/main/llvm/tools/llvm-cov/CoverageExporterLcov.cpp) emits branch summaries when branch export is enabled, even if compilation supplied no branch records. `tests/coverage_llvm_live.rs` compiles and executes a Rust conditional with the pinned line-coverage instrumentation, merges its profile with LLVM and exports LCOV. It verifies the resulting `BRF:0`/`BRH:0` report cannot satisfy required branch measurement. The same report passes an explicitly line-only contract. This fixture runs in the independent `coverage-producers` job using the pinned `llvm-tools-preview` component.

An LCOV report must contain positive branch records and consistent `BRF`/`BRH` summaries for every section to establish branch measurement. Earlier zero-only LCOV acceptance is deliberately tightened: use explicit line-only policy or a producer contract carrying independently established measurement evidence. Zero-only counts cannot establish a branch-free source program by themselves.

## Requirement-to-test evidence

| Requirement | Verification |
|---|---|
| §3.2 complete coverage facts and branch thresholds | `adapters/reports/coverage_tests.rs`, `tests/coverage_live.rs` |
| §3.5 changed-line/staged scope and missing sources | `application/report_gate_tests.rs`, `tests/coverage.rs` |
| §5.2 bounded native adapters | DTD/entity, XML-node, hierarchy, duplicate identity, source-root and repeated-filename fixtures in `coverage_tests.rs` |
| §6.3 violations versus incomplete measurement | Native summaries, zero-branch/disabled measurement, timeout and real test/compiler failure scenarios |
| §6.6 policy changes and §8 immutable evidence | Trusted-policy, staged-byte, input-mutation and retained raw artifact assertions in `tests/coverage.rs` |
| §9.1 real producer repair | JDK 21/JaCoCo 0.8.15 and Python 3.12/coverage.py 7.10.7/pytest 8.4.2 fixtures in the independent `coverage-producers` CI job |

The JaCoCo distribution is pinned by SHA-256 `5b3f6ddb724e761d25c937d68b0189a3a23f3e220e3282575ee0b53359e8110e`. The Rust harness downloads it with byte/time bounds or accepts the same verified archive through `QUALITYGATE_TEST_JACOCO`. `QUALITYGATE_TEST_JAVA`, `QUALITYGATE_TEST_JAVAC` and `QUALITYGATE_TEST_COVERAGE_PYTHON` select provisioned tools. All project inputs and histories are temporary. These fixtures verify producer interoperability; measured team-pilot acceptance remains separate.
