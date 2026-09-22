# Projects and tool reports

[简体中文](../../zh/02-reference/03-projects-and-tool-reports.md) · [Volume index](README.md)

Project adapters provide facts that cannot be inferred safely from a single
source file. Maven analysis resolves modules, declared dependencies, producer
relationships, and compiled usage evidence. Python analysis compares installed
distributions with declared requirements and retains environment/interpreter
identity. Missing manifests, ambiguous module ownership, unresolved artifacts,
or unsupported requirement syntax remain incomplete.

## Unified report contract

External checks declare a fixed command, working directory, timeout, accepted
statuses, tool/version evidence, report paths, parser profile, and mode. Reports
are read only from the materialized snapshot execution and have bounded size,
file count, diagnostics, and parse time. Missing, stale, malformed, truncated,
or snapshot-mismatched reports never pass silently.

The normalized result retains tool and rule IDs, severity, message, URI,
location, fingerprints, counts, raw report digest, command evidence, and
completeness. Exit code and report content are evaluated together; an accepted
producer exit code cannot override a fatal or incomplete report.

## Coverage and static analysis

Coverage producers support native coverage.py JSON, Cobertura XML, JaCoCo XML,
and LCOV within documented profiles. Thresholds use covered and total line
facts; zero counters are retained. Empty or internally inconsistent reports,
missing source mappings, duplicate incompatible entries, and parse limits are
incomplete. A high percentage is not proof of behavior.

SARIF support preserves runs, tool versions, rules, result kinds, messages,
locations, and partial fingerprints. A ratchet reruns the same analyzer against
both immutable snapshots and compares `(tool, rule)` counts or stable identities
as configured. Suppressed, absent, or baseline-only findings stay distinguishable.

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

Coverage metadata `<report>:mode` records the effective measurement mode:
delivery uses `changed_lines`, including when policy configures `full`.
`<report>:configured_mode` retains the configured value. Repository scope keeps
the configured coverage mode. Counts and thresholds follow the effective mode.
