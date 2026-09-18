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
PMD, SpotBugs, SARIF and Cargo Clippy JSON Lines. A baseline path is mandatory. Test statistics and
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
