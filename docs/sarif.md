# SARIF static-analysis reports

Use `format: sarif` for fresh, self-contained SARIF 2.1.0 output from a configured analyzer command. The [common report contract](reports.md) supplies executable version probes, immutable current/base workspaces, deadlines, exit-code validation and retained raw artifacts. A SARIF document cannot replace that execution evidence.

```yaml
schema_version: 1
checks:
  - id: static-analysis
    argv: [./analyze, --sarif, target/analysis.sarif]
    timeout_seconds: 300
    tools:
      - id: analyzer
        argv: [./analyze, --version]
        inputs: [analyze]
    reports:
      - path: target/analysis.sarif
        format: sarif
        mode: new_diagnostics
        baseline: target/analysis.sarif
```

`analyze` represents the project's existing analyzer entry point. Its version probe must identify the tools that actually produce the report; declare nested tools separately when using a wrapper. The baseline uses the same command and policy against the comparison's base commit. Both executions must complete with comparable tool identities and intact snapshot inputs.

## Completed analysis and result kinds

Each run must identify `tool.driver.name` and contain an explicit `results` array. An empty array is a completed scan with no results; absent/null results are incomplete evidence. If `invocations` is supplied, every entry must contain boolean `executionSuccessful: true`. Error notifications or exceptions in either execution or configuration notifications invalidate completion. Missing invocations are permitted because the outer command supplies independent execution evidence.

The adapter interprets `kind: fail` (also the default) as a finding. `pass`, `notApplicable` and `informational` are counted without becoming violations. `open` and `review` have unresolved outcomes and make validation incomplete. Invalid kinds, levels and inconsistent non-failing severities are rejected. The configured check severity controls gate blocking; an analyzer's note/warning level cannot weaken that policy.

Reported suppressions remain findings. A SARIF `accepted` suppression does not authorize a policy waiver. Metadata under `<report-path>:sarif_runs` records producer name, reported version, total results, violations, non-violations and suppressed-result counts. These reported identities supplement the separately executed `metadata.tools` probes; they are not proof of the executable's provenance.

These interpretations follow the [OASIS SARIF 2.1.0 specification](https://docs.oasis-open.org/sarif/sarif/v2.1.0/os/sarif-v2.1.0-os.html), particularly runs/results (§3.14.23), invocation completion (§3.20.14), result kinds (§3.27.9), messages (§3.11), artifact locations (§3.4) and regions (§3.30). Qualitygate applies the explicit policy choices and supported profile documented here; it is not a complete SARIF schema validator or viewer.

## Locations, messages and incremental identity

Rule IDs and driver rule-table indices, artifact-table indices, chained `originalUriBaseIds`, and indexed logical locations are resolved before gate filtering. Contradictory IDs/indices, unresolved references and cycles invalidate the report. Messages use plain `text` or driver rule/global message catalogs, expand indexed arguments, and decode escaped braces. Required plain text cannot be replaced by Markdown alone.

File locations support percent-encoded relative URIs and local absolute file URIs. A relative URI without a base ID is rooted at the checked repository; analyzers invoked in nested directories should emit an explicit base or repository-relative paths. Absolute locations must map to checked snapshot files. Remote hosts, non-file schemes, query/fragment suffixes and paths that escape the checked sources are rejected. Paths are decoded before source mapping; spaces and native Windows file URIs are supported. Encoded backslashes are rejected before native conversion so Windows and Unix cannot interpret the same URI as different paths.

A result becomes one diagnostic containing all distinct primary locations in `evidence.locations`. Positive start/end lines must fit the exact selected source file. `changed_lines` includes a result if any primary location's inclusive line range overlaps added/changed lines and displays that matching location. An unlocated result cannot silently disappear when no other location proves a match. `full` can retain file-only or logical-only results; `changed_lines` requires actual line evidence. Related locations and code-flow steps remain in the raw artifact and do not substitute for primary locations.

For SARIF, stable identity combines producer name, rule ID, the normalized set of file/symbol identities, and the expanded message. Location order, display selection and line movement do not change it. Base-side file renames are mapped before comparison; repeated identical results use multiplicity. A different producer, symbol or message constitutes a different identity. Analyzer-supplied `fingerprints` and `partialFingerprints` do not replace this independently computed comparison. Legacy single-location generic diagnostics retain their existing identity algorithm.

## Supported profile and bounds

External property bundles, inline external properties, pre-baselined `baselineGuid`/`baselineState`, extension rule references and binary addresses require additional binding/resolution and currently produce incomplete validation. Generate complete fresh results for each qualitygate execution. Character/byte offsets are not converted into source lines; a report without line coordinates cannot satisfy a changed-line filter.

Reports are limited to 2 MiB. Additional parser budgets are 50,000 results per run, 256 primary locations per result, reference depth 32, 200,000 location-resolution steps per document, 16 KiB per resolved URI, 2 MiB per expanded message, and 16 MiB of serialized normalized evidence including repeated producer/rule names. Exceeding a budget makes validation incomplete. Readable reports within the input-size limit are retained before normalization; oversized or unreadable reports retain command logs and failure details instead. Raw URI control characters and surrounding whitespace are rejected before URL parsing can discard them.

The generic `diagnostics` format also accepts optional `tool` and `locations` on an issue. Each location contains optional `file`, `line`, `end_line` and `symbol`; when supplied, the first location must agree with the issue's primary `file`, `line` and `symbol`. Ranges obey the same source bounds. Multi-location issues use the same order-independent identity and selection logic, including `affected_scope` with explicitly supplied `affected_files`.

## Executable evidence

`tests/sarif.rs` uses isolated Git repositories and a Rust producer to verify indexed absolute paths across fresh workspaces, baseline filtering, location-order stability, ranges, producer identity, suppression policy and incomplete reports. Parser and report-gate unit tests cover malformed inputs, resolution budgets and source bounds.

`tests/sarif_live.rs` runs real pinned Clippy through [clippy-sarif 0.8.0](https://docs.rs/clippy-sarif/0.8.0/clippy_sarif/). The Rust fixture driver checks compiler completion before conversion and bounds intermediate output. The acceptance loop preserves a historical warning through line movement, detects and repairs a new warning, and rejects compiler failure. The `sarif-project` CI job installs the pinned converter and explicitly executes this otherwise ignored external-tool fixture.

```bash
cargo install clippy-sarif --version 0.8.0 --locked
cargo test --locked --test sarif_live -- --ignored --nocapture
```

For an isolated converter installation, set `QUALITYGATE_TEST_CLIPPY_SARIF` to its executable path. These controlled acceptance cases establish tool-format interoperability; [real pilot measurement](pilot.md) remains a separate requirement.
