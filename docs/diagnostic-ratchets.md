# Diagnostic count ratchets

`reports[].mode: ratchet` allows historical diagnostic debt to decrease or hold,
and fails when any tool/rule count grows. The same analyzer runs on both
immutable snapshots. The baseline is freshly produced from the comparison's
base commit; an editable count file or historical report is never trusted.

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

Replace the example producer with the project's existing analyzer and adopt its
actual command, tool inputs, timeout and findings exit codes through normal
policy review. Diagnostic formats include generic `diagnostics`, Checkstyle,
PMD, SpotBugs, SARIF, Cargo Clippy JSON Lines, ESLint JSON, golangci-lint v2 JSON and Ruff JSON. A baseline path is mandatory. Test statistics and
coverage cannot use this mode, even when supplied inside generic JSON.

## Clippy from captured stdout

The packaged [Clippy reference policy](../skills/qualitygate-cli/references/clippy-ratchet.yaml)
uses `cargo clippy --message-format=json` and `format: cargo_clippy` with
`from_stdout: true`. [Cargo emits one JSON record per line to stdout](https://doc.rust-lang.org/cargo/reference/external-tools.html); it does
not create `target/clippy.jsonl`. That confined path is the report identity
used in evidence and ratchet metadata. Qualitygate retains the bounded raw
stdout as a report artifact on both snapshots. A missing `build-finished`
record, malformed JSON, compiler error, unrecognized analyzer exit or source
path outside the checked snapshot makes the run incomplete.

`--lib` checks a library's `src/` target without test and benchmark targets.
For a binary crate, use `--bin NAME`; for all binaries use `--bins`. These Cargo
target selectors do not filter diagnostics by pathname, so keep tests and
benches out of the command if the debt policy covers only production targets.
Use `-W` for lint levels managed by the ratchet; `-A` suppresses selected
lints, and `-D` makes Clippy fail on findings. A policy using `-D` must declare
its observed nonzero lint exit in `findings_exit_codes` (usually `101`), while
compiler errors still remain incomplete. Qualitygate's check `severity`
controls whether ratchet growth blocks; it does not set Clippy lint levels.
Choose lint groups or IDs with [Clippy command flags, source attributes, or Cargo's
`[lints.clippy]`](https://doc.rust-lang.org/clippy/configuration.html);
`clippy.toml` configures lint behavior such as thresholds.
Pin the Rust toolchain in `rust-toolchain.toml`, keep `Cargo.lock`, and retain
the `cargo clippy --version` probe. The current and baseline runs must report
comparable versions and executable identities. Lint IDs can change across
toolchain versions, so review upgrades as policy changes.

## ESLint from captured stdout

The packaged [ESLint reference policy](../skills/qualitygate-cli/references/eslint-ratchet.yaml)
uses a provisioned `eslint` executable with `--format=json` and
`--exit-on-fatal-error`, `findings_exit_codes: [1]`, and an `eslint_json`
stdout report. [ESLint's JSON formatter](https://eslint.org/docs/latest/use/formatters/)
emits file results, not the generic `diagnostics` envelope. ESLint exits 1
for lint errors and 2 for configuration or internal failures, per its
[CLI contract](https://eslint.org/docs/latest/use/command-line-interface).
Qualitygate rejects empty file inventories, malformed or contradictory counts,
fatal parser messages, unmappable locations and unrecognized exits as
incomplete. The report path is a confined identity for captured stdout, not a
file ESLint writes. Both raw reports remain in the check artifacts.

`src` in the reference command limits the analyzer's declared targets;
configure [flat config `files` and `ignores`](https://eslint.org/docs/latest/use/configure/configuration-files)
for the actual `.js` and `.ts` source inventory. ESLint rule severity
`off` removes a finding, `warn` reports it without nonzero exit, and `error`
returns 1; Qualitygate's check severity controls whether ratchet growth
blocks. Rule selection and inline suppression belong to ESLint config and are
reviewed as source inputs. Do not use `--quiet`, `--cache`, or `--fix` for a
complete fresh report. Provision a pinned ESLint and any parser/plugins for
both materialized snapshots; the version probe and executable digest must
match. A project may use its own lockfile-backed setup, but a missing
dependency cannot be replaced by a clean report.

## golangci-lint v2 from captured stdout

The packaged [Go reference policy](../skills/qualitygate-cli/references/golangci-lint-ratchet.yaml)
uses [golangci-lint v2 output flags](https://golangci-lint.run/docs/configuration/cli/#run)
to send JSON to stdout and text to stderr. `--show-stats=false` keeps stdout a
single JSON document. `--max-issues-per-linter=0`, `--max-same-issues=0`, and
`--uniq-by-line=false` preserve the full issue count needed for a ratchet.
`findings_exit_codes: [1]` accepts lint findings; other nonzero exits remain
incomplete. The adapter rejects analyzer errors, warnings, typecheck issues,
missing enabled-linter inventory, invalid locations and malformed JSON. It
uses the linter name as `tool`, an explicit rule code such as `SA5009` or `G101`
when present as `rule`, and otherwise the linter name as `rule`.
Qualitygate's configured check `severity` decides whether count growth blocks;
golangci-lint's per-issue `Severity` remains in the retained raw report and
does not silently override that policy.

Keep v2 linter selection in a tracked `.golangci.yml`; the command's final
`./...` can be narrowed to a package path, while test selection belongs in
the Go analyzer configuration or `--tests=false`. Use a provisioned, pinned
binary and Go toolchain for both snapshots. Avoid `--new`, `--fast-only`,
`--fix` and count-limiting flags: these change the observed debt or source.
The reference uses `--modules-download-mode=readonly` to keep module manifests
unchanged. Cache paths should be isolated from unrelated runtime state; raw
JSON and tool/version evidence remain attached to each check.

## Ruff from captured stdout

The packaged [Ruff reference policy](../skills/qualitygate-cli/references/ruff-ratchet.yaml)
uses `ruff check --output-format=json` and `format: ruff_json` with captured
stdout. [Ruff returns 1 for findings and 2 for abnormal execution](https://docs.astral.sh/ruff/linter/#exit-codes),
so the reference accepts only `findings_exit_codes: [1]`. The adapter requires
rule codes, messages, filenames and valid source ranges; syntax and I/O errors
are incomplete analysis rather than ratcheted debt. Each rule code is counted
under `tool: ruff`. Ruff's `severity` does not override Qualitygate's check
severity. Both raw reports and executable/version evidence are retained.

Ruff JSON returns findings but no checked-file inventory. A clean `[]` means
Ruff reported no findings for its selected targets; review the explicit final
path argument and tracked `ruff.toml` or `pyproject.toml` excludes to establish
scope. Change `.` to `src/` or specific packages when the ratchet should omit
tests or environments. Pin the Ruff executable for both snapshots. `--no-fix`
and `--no-fix-only` prevent configured fixes from rewriting the materialized
source; `--no-cache` forces fresh analysis. Avoid `--exit-zero`,
`--statistics`, `--add-noqa`, `--add-ignore` and filtered `--select` overrides
that would change the debt being compared without a policy review.

Counts preserve multiplicity and are grouped by `(tool, rule)` independently
within each report. Formats without tool names use `null` for that key component.
Missing baseline buckets start at zero; disappearing buckets record current
zero. A reduction under one rule cannot offset growth under another rule/tool.
Equal counts may contain different issues: use `new_diagnostics` when identity
replacement must also fail. Existing modes retain their meaning.

For baseline `a=2, b=0`, current `a=1, b=1` fails for `b`; current `a=1, b=0`
passes and records the improvement. Once the improvement becomes the selected
comparison base, returning to `a=2` fails. There is no slack or reset command.

Metadata `<path>:ratchet` is a deterministic array of objects containing
`key: {tool, rule}`, `baseline` and `current`. All current diagnostics in an
increased bucket are emitted for repair; other findings contribute to
`<path>:filtered`. Both raw reports and execution logs remain in the durable
artifacts. Measurements describe observed debt even when the gate passes.

The [execution contract](reports.md#exit-code-and-baseline-semantics) still
requires comparable tool versions, executable digests and declared tool inputs,
intact materializations, fresh reports and recognized exits. Missing/malformed
reports, timeout, source mutation or incomparable tools produce exit 2. Complete
growth produces exit 1 at error severity; warning severity retains normal
warning semantics. Historical and current locations are validated even when
their findings would otherwise be filtered out.

Counts depend on the producer's declared scope. A valid empty diagnostic report
means the producer reported no findings; it does not prove comprehensive source
analysis or test effectiveness. Scoped CLI checks do not establish delivery
readiness. Existing policy-reference and artifact bounds still apply.

The count decision belongs to the pure domain. Strict configuration, adapter
normalization, application orchestration and bounded runner execution keep
their current ownership. No new top-level owner or dependency direction is added.

| Requirement | Regression evidence |
|---|---|
| Independent buckets, deterministic order, zero/decreasing counts | `domain::ratchet::tests::debt_decreases_never_hide_growth_in_another_rule_or_tool` |
| Increased findings, filtered-location validation, non-diagnostic rejection | `application::report_gate::tests::ratchet_counts_each_rule_and_tool_and_validates_even_filtered_locations` |
| Fresh paired runs, multiplicity, staged isolation, lowered baselines | `tests/ratchet.rs::ratchet_reexecutes_both_snapshots_and_does_not_trade_between_rule_counts` |
| Missing report, timeout, mutation, unexpected exit, tool/policy mismatch | `ratchet_baseline_failures_timeouts_mutation_and_tool_mismatch_are_incomplete` |
| Malformed current/base reports and strict configuration | `ratchet_malformed_and_non_diagnostic_reports_cannot_become_zero_debt` |

Run `cargo test --all-features --test ratchet` separately from native unit tests.
The Rust integration producer operates only in temporary repositories.
