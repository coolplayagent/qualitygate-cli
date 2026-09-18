# Prevent diagnostic debt growth

Use `reports[].mode: ratchet` when the requested policy allows existing analyzer
debt to hold or decrease but forbids count growth. It configures a command
check, not a project-rule DSL assertion. Use `new_diagnostics` instead when
replacement with different issues must also fail: equal ratchet counts may
contain different findings. Preserve an existing stricter policy unless the
user has authorized changing it.

## Configure a comparable analyzer

Read the effective configuration and the project's actual analyzer invocation.
For an authorized candidate edit, merge a report definition into its check,
preserving unrelated settings. This example must use the project's real
command, output path, version probe, inputs, timeout and findings exit codes:

```yaml
schema_version: 1
checks:
  - id: diagnostic-debt
    argv: [project-analyzer, --output, target/analysis.json]
    tools:
      - id: analyzer
        argv: [project-analyzer, --version]
    reports:
      - path: target/analysis.json
        baseline: target/analysis.json
        format: diagnostics
        mode: ratchet
```

Supported diagnostic formats are `diagnostics`, `sarif`, `checkstyle`, `pmd`,
`spotbugs`, `cargo_clippy`, `eslint_json`, `golangci_json` and `ruff_json`. For Clippy, use the bundled
[reference policy](clippy-ratchet.yaml): `cargo clippy --message-format=json`
emits JSON Lines to captured stdout, so set `from_stdout: true` and use a
confined `path` only as the report identity. `baseline` is mandatory in the
base workspace, usually equal to `path`; it is not a stored count file or a Git
reference. Test statistics, `minimum_tests` and coverage are incompatible with
this mode, including test/coverage data inside generic JSON.

For Clippy, select `--lib`, `--bin NAME` or `--bins` to restrict targets to
production `src/` code. Use `-W` with ratchet for nonfatal lint findings; `-A`
suppresses lints, and `-D` requires a declared findings exit code. Lint
selection belongs in flags, source attributes or Cargo's `[lints.clippy]`;
`clippy.toml` tunes lint configuration. Pin the Rust toolchain and probe
`cargo clippy --version`. A malformed stream, failed compilation or missing
`build-finished` record is incomplete.

For ESLint, use the bundled [reference policy](eslint-ratchet.yaml). The
`--format=json` command emits a file-result array to stdout, so set
`from_stdout: true`, `format: eslint_json`, and `findings_exit_codes: [1]`.
Use `--exit-on-fatal-error` so parse errors exit 2. The `src` target and the
project's flat config establish analyzed scope, rule selection and ignores;
the rule counts are not a substitute for complete scope evidence. Provision
a pinned ESLint executable and plugins for both snapshots, retain its version
probe, and do not use cache, quiet or fix modes for a fresh ratchet report.

For Go, use the bundled [golangci-lint v2 reference policy](golangci-lint-ratchet.yaml).
It sends JSON to captured stdout and text to stderr, disables stdout stats,
count limits and line deduplication, and declares `findings_exit_codes: [1]`.
Track `.golangci.yml` for linter selection and edit the final package target
for scope. Keep the Go toolchain and golangci-lint binary pinned for both
snapshots. Analyzer warnings, errors, typecheck issues and invalid locations
are incomplete; do not use `--new`, `--fix` or `--fast-only` in the ratchet.

For Python, use the bundled [Ruff reference policy](ruff-ratchet.yaml): JSON
findings come from captured stdout with `findings_exit_codes: [1]`. Ruff
configuration owns rule selection and excludes; an explicit final path owns
target scope. The adapter rejects syntax and I/O diagnostics and malformed
locations as incomplete. Ruff JSON has no checked-file inventory, so `[]`
requires reviewing the target path and tracked config. Pin Ruff for both
snapshots and keep `--no-fix --no-fix-only --no-cache` for fresh, nonmutating
analysis.

Declare report-producing tools and stable version probes, plus `tools[].inputs`
for repository analyzer scripts, wrappers or other tool assets. Both runs need
matching versions, executable digests and declared input digests. Generated
report paths must not be tracked source inputs. The CLI removes old outputs
before execution; an analyzer must emit a fresh, complete supported report.

Validate the candidate with the selected runtime before running the analyzer:

```bash
"$QUALITYGATE_BIN" --root "$REPOSITORY_ROOT" config --show --format json
```

If an active immutable policy exists, ordinary reads/checks select that package;
use the authorized candidate workflow in [rule management](rule-management.md)
to validate the edited candidate, and verify the reported policy identity.
An unknown mode or incompatible configuration is a validation gap. Do not
delete `ratchet` or fall back to exit-code-only checks to make it accepted.
Configuration success does not approve policy adoption or prove execution.

## Execute and interpret the comparison

Run the caller-selected snapshot and policy via [operations](operations.md).
The CLI runs the same analyzer on the selected comparison base and current
snapshot in separate materializations. Keep this base distinct from
`--policy-ref`, which selects policy. Do not invent a new base or replace a
baseline artifact to reset observed debt.

Each report counts diagnostic multiplicity by `(tool, rule)`. Formats without
a tool name use `null`; missing buckets have zero findings. Growth in any
bucket cannot be offset by a decrease elsewhere. For base `a=2, b=0`, current
`a=1, b=1` fails for `b`, while `a=1, b=0` passes. Once that improvement is the
selected comparison base, returning to `a=2` is growth. There is no slack or
reset operation.

Read `metadata["<report-path>:ratchet"]`, an ordered array of
`{key: {tool, rule}, baseline, current}` measurements. All current diagnostics
from increased buckets are emitted for repair; other findings count toward
`<report-path>:filtered`. Keep both raw reports, base snapshot/tool evidence
and execution logs from `execution.artifacts`. Repair the reported issues and
use the pinned recheck with the same policy, comparison and scope.

At error severity, complete count growth produces exit 1; warning severity
keeps its ordinary warning semantics. Missing/malformed reports, timeouts,
source mutation, invalid locations, unrecognized exits or incomparable tools
produce incomplete execution (exit 2), even if current counts appear lower.
Locations are validated in both reports before filtering. A valid empty report
means the analyzer reported zero findings, not that it scanned everything.
Retain producer-scope limits and the CLI verification fields in the result;
scoped checks and synthetic fixtures do not prove delivery readiness.
