# Projects and tool reports

[简体中文](../../zh/02-reference/03-projects-and-tool-reports.md) · [Volume index](README.md)

Project adapters provide facts that cannot be inferred safely from a single
source file. Maven analysis resolves modules, declared dependencies, producer
relationships, and compiled usage evidence. Python analysis compares installed
distributions with declared requirements and retains environment/interpreter
identity. Missing manifests, ambiguous module ownership, unresolved artifacts,
or unsupported requirement syntax remain incomplete.

Installed Python `METADATA` entries must be regular files of at most 2 MiB.
Collection checks file type and size before opening, validates the opened handle,
and keeps a bounded read to detect growth. An existing FIFO, socket, directory,
or oversized entry makes the producer incomplete even after a successful install
command. Collected command logs and the install report remain available; dependent
checks stay blocked. This preflight does not provide an OS filesystem deadline or
protection against every concurrent path replacement. Live pip acceptance is
verified separately from synthetic malformed-output regressions.

## Unified report contract

External checks declare a fixed command, working directory, timeout, accepted
statuses, tool/version evidence, report paths, parser profile, and mode. Reports
are read only from the materialized snapshot execution and have bounded size,
file count, diagnostics, and parse time. Missing, stale, malformed, truncated,
or snapshot-mismatched reports never pass silently.

All XML adapter entrypoints share a maximum element depth of 32, counting the
root, and a 100,000-node limit, including the document root. This covers JUnit,
Checkstyle, PMD, SpotBugs, Cobertura, JaCoCo, paired JUnit, Maven snapshot manifests,
effective models and usage analysis, and japicmp comparison and API inventory.
An iterative token preflight checks depth before the recursive tree parser runs.
Comments, CDATA, processing instructions and escaped attribute values do not add
depth. Each caller retains its byte budget: generated reports/models are limited
to 2 MiB; snapshot manifests use the selected snapshot acquisition budget.
Excessive depth or node counts return incomplete evidence (exit 2); collected
raw reports remain available and dependent checks stay blocked. Report-specific
DOCTYPE handling remains with the report profile.

The normalized result retains tool and rule IDs, severity, message, URI,
location, fingerprints, counts, raw report digest, command evidence, and
completeness. Exit code and report content are evaluated together; an accepted
producer exit code cannot override a fatal or incomplete report.

Declared report and project-output files must not overlap checked snapshot
inputs. Normalized path conflicts, including `./` aliases and baseline outputs,
are rejected before report preparation changes files; `doctor` reports the
corresponding current or baseline conflict. A stdout report's path is only an
evidence label and does not reserve a writable file. Input integrity is verified
after tool probes and report preparation, before the producer starts. Generated
outputs from earlier commands are still cleared, so each producer must supply
fresh reports. Keep generated reports outside checked inputs, for example in an
ignored build directory.

Within a check, report paths (including stdout labels) and project-output paths
must be distinct after repository-relative normalization: `target/report.json`
and `./target//report.json` name the same output. Configuration loading rejects
duplicates before probes or producers run. When baseline execution is required,
its file outputs, including current-path fallbacks, are checked independently;
current and baseline runs may use the same path in their separate workspaces.
A single valid alias remains accepted, and declared paths are not rewritten.

Test counts and selected coverage-line counts contribute to a check's
`matched_entities`. Each addition must fit the runtime's unsigned counter;
overflow makes the check incomplete instead of wrapping or panicking. Earlier
findings and raw report artifacts remain available, and dependent checks stay
blocked until the producer supplies representable evidence. Debug and release
builds apply the same rule.

JUnit validates every supplied `tests`, `failures`, `errors` and `skipped`
counter against the testcase records in that suite, including nested suites.
Counters remain optional. A testcase must belong directly to a testsuite and
have one outcome kind; multiple failure records count as one failed testcase.
Contradictory counts or outcomes, duplicate errors/skips and detached outcome
records make validation incomplete (exit 2), retain the raw report and block
dependents. Consistent failing cases remain violations (exit 1 when all required
checks complete; a blocked required dependent still yields exit 2). Ordinary JUnit
still accepts missing file, classname, name and failure-type metadata; paired
test-effectiveness evidence keeps its additional identity and source requirements.

## Coverage and static analysis

Coverage producers support native coverage.py JSON, Cobertura XML, JaCoCo XML,
and LCOV within documented profiles. Thresholds use covered and total line
facts; zero counters are retained. Empty or internally inconsistent reports,
missing source mappings, duplicate incompatible entries, and parse limits are
incomplete. A high percentage is not proof of behavior.

LCOV summary mismatches identify the `SF` source, counter (`LF`, `LH`, `BRF`
or `BRH`), declared value and value derived from the detailed records. The raw
report is retained and the check remains incomplete. This also applies to real
producer inconsistencies: the pinned Rust/LLVM exporter can report `LF:5` for
four `DA` records when a closure overlaps its enclosing function. Inspect the
named source section and regenerate consistent producer evidence; changing a
coverage threshold does not resolve a contradictory report.

Coverage decisions compare exact covered/total counts with the configured
threshold's canonical decimal value before rounding display percentages. A
100% threshold requires every measured item to be covered; a 99.9% threshold
accepts exactly 999/1000. Large counts cannot pass by rounding a deficit away.
Diagnostics include exact counts even when the displayed percentage rounds to
the threshold. Empty measured inventories retain null percentages.

SARIF support preserves runs, tool versions, rules, result kinds, messages,
locations, and partial fingerprints. A ratchet reruns the same analyzer against
both immutable snapshots and compares `(tool, rule)` counts or stable identities
as configured. Suppressed, absent, or baseline-only findings stay distinguishable.

If baseline execution, input validation or report parsing fails, current file
and stdout reports are still collected within the usual byte limits and retained
as raw artifacts. They are not compared against an empty baseline: the check
remains incomplete and dependents remain blocked. If current report collection
also fails, its error is retained together with the baseline failure. Reports
successfully collected before that error remain available. This applies to both
`ratchet` and `new_diagnostics` checks.

Reference ratchets cover Clippy, ESLint, golangci-lint, Ruff, GCC/Clang analyzer
SARIF, Checkstyle, PMD, and SpotBugs. Their producer-specific completion markers,
file inventories, error records, path mappings, and version agreement are part
of completeness, not optional metadata.

## Compatibility, Bazel, and incremental meaning

Java compatibility checks build paired snapshots and use configured binary and
source compatibility tooling. Failure to build either side, resolve the API
surface, or run the selected tool is incomplete.

Bazel is an additional reproducible build surface using Bzlmod and Cargo-aligned
dependencies; it does not replace Cargo's required quality gates. Cache state,
lockfile behavior, platform, and selected targets belong in evidence.

Incremental modes declare their meaning: `new_diagnostics`, `changed_lines`,
`affected_scope`, or `full`. A changed file does not make every historical
finding new. When a comparable baseline is unavailable, a required ratchet is
incomplete unless trusted policy explicitly allows a disclosed full fallback.

For `new_diagnostics`, unchanged current occurrences consume matching baseline
counts before selecting delivery findings. Adding an identical finding on a
changed line therefore remains new when the historical occurrence is retained;
moving an existing occurrence alone does not make it new.

Coverage metadata `<report>:mode` records the effective measurement mode:
delivery uses `changed_lines`, including when policy configures `full`.
`<report>:configured_mode` retains the configured value. Reusable-core repository evaluations keep
the configured coverage mode. Counts and thresholds follow the effective mode.

---

[Previous: Built-in and custom rules](02-rules.md) · [Contents](../README.md) · [Next: Policy, task, and signed evidence](04-policy-task-and-signed-evidence.md)
