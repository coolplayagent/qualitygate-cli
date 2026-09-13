# Tool report contract

Command checks run argv directly inside a materialized snapshot. Configure `cwd`, `timeout_seconds`, `expected_exit_code`, `findings_exit_codes`, `required_args`, `depends_on`, `tools`, and `reports`. Standard output, standard error and generated reports are saved as digest-bearing evidence. A prerequisite must complete and pass before dependents run.

Supported report formats are `junit`, `checkstyle`, `spotbugs`, `pmd`, `sarif` (2.1.0), `lcov`, `cobertura`, `jacoco`, `coverage_py` (native coverage.py JSON v2/v3), and `diagnostics` (the JSON contract below). Missing, empty, malformed or oversized reports make checks incomplete. Configured output files are removed from the isolated workspace before execution, preventing a previous report from masquerading as a new run. Do not track generated report paths as checked source inputs.

The [SARIF contract](sarif.md) specifies supported URI/index resolution, result kinds, multi-location ranges, stable identities and incomplete external/pre-baselined reports. Diagnostic line ranges must fit the selected source snapshot before any incremental filtering.

```yaml
schema_version: 1
checks:
  - id: java-tests
    kind: command
    argv: [./mvnw, -s, settings.xml, test]
    timeout_seconds: 300
    required: true
    severity: error
    findings_exit_codes: [1]
    tools:
      - id: maven
        argv: [./mvnw, --version]
        inputs: [.mvn/wrapper/maven-wrapper.properties]
    reports:
      - path: target/surefire-reports/TEST-com.example.OrderTest.xml
        format: junit
        minimum_tests: 1
```

JUnit counts actual testcase records and excludes skipped cases from executed tests. Its default minimum is one, and `minimum_tests: 0` is invalid. Assertion failures remain completed failing checks; generic reports declaring failed tests also fail even if their diagnostic list is empty.

For ordinary `cargo test` commands, the CLI can also aggregate Rust test-harness summaries from captured stdout. Zero executed tests fail; missing counts make validation incomplete. Recognized Maven, Gradle, Go and pytest test commands require a configured test-count report rather than relying solely on exit status. Partial process logs are retained on timeout or output-budget failure.

A command missing its configured `required_args` is blocked without execution, including when its diagnostic severity is warning. Static argument inspection cannot establish that the command ran.

## Execution and tool evidence

Every executed command records its resolved executable path and SHA-256 content identity, original and resolved argv, snapshot, start/end times, exit code and duration. Executable identity reads are limited to 256 MiB. The environment record contains OS, architecture, CLI version and hashes of recognized dependency manifests and lockfiles from the snapshot. Arbitrary environment variables and credentials are not collected. Executable resolution follows the command's working directory and platform path lookup through [which_in](https://docs.rs/which/8.0.6/which/fn.which_in.html).

Report-producing checks require declared `tools` with version commands. Each probe executes in the same workspace and working directory as the check; its executable identity, actual version response, timings, exit status, output logs and snapshot digest are recorded. A configured version string cannot replace execution. Probes default to a 10-second deadline, configurable with `timeout_seconds` from 1 to 60. Successful version output must be nonempty UTF-8 without NUL bytes and at most 64 KiB in total. Failed, timed-out, missing or invalid version evidence prevents the check from running. Such gaps retain the check's required/optional completeness semantics.

Use `tools[].inputs` to bind analyzer scripts, JARs, wrapper configuration or other repository-owned tool assets to their actual snapshot contents. Paths are repository-relative and must exist in that snapshot. Declare the tools that generate the reports, including tools invoked inside wrapper scripts. Binary content identities identify directly executed programs; they do not infer every nested tool or seal the host's complete dependency/runtime environment.

Version-probe logs live in each tool's `stdout`/`stderr` artifact records. Main and baseline logs/reports live in `execution.artifacts`. Artifact names derive from check IDs so case differences and platform-reserved filenames cannot overwrite another check's evidence. Tool executables are checked again before and after execution; detected identity changes invalidate the result.

Captured source, configuration, test and build-input files are checked before and after each command and independently around baseline execution. Content changes, replacement/deletion, symbolic-link substitution, metadata changes and Unix executable-bit changes invalidate that materialization permanently. Restoring bytes later cannot validate earlier results, and subsequent commands do not run against an invalid workspace. Metadata is checked as well as bytes to detect ordinary rewrite-and-restore operations within a command. Windows does not expose Unix executable-bit semantics. This is input integrity validation, not an operating-system sandbox or an adversarial filesystem event audit; generated files outside the captured input inventory remain command outputs.

Source snapshot or provider revalidation failures retain the original comparison, completed diagnostics and execution evidence in an incomplete report.

Materialized workspaces expose a physical canonical root using [dunce](https://docs.rs/dunce/1.0.5/dunce/fn.canonicalize.html), so tools returning physical paths do not disagree with macOS temporary-directory aliases. The directory owner still handles cleanup. Windows input guards additionally compare volume/file identity through [file-id](https://docs.rs/file-id/0.2.3/file_id/), because timestamps alone may not distinguish a rapid same-content replacement. These checks retain the integrity limits described above.

## Exit-code and baseline semantics

`expected_exit_code` defaults to zero. Tools that use a nonzero code for ordinary findings can declare `findings_exit_codes`, for example `[1]`; this requires reports. Those codes are accepted as completed analyzer execution only when all required reports are generated, validated and contain corresponding findings before incremental filtering. A findings exit code with wholly clean reports is inconsistent evidence and makes the check incomplete. Report diagnostics and the chosen increment mode determine the verdict. Other nonzero exits fail plain command checks; unrecognized analyzer exits and signal termination make report-producing checks incomplete. Task command contracts support the same expected/findings exit codes and tool declarations.

For `new_diagnostics`, the base run must have a declared successful/findings exit code, intact inputs and comparable tool evidence. Baseline/current version responses, executable digests and declared tool-input digests must match. A valid-looking report cannot compensate for a crashed analyzer, an unrecognized baseline exit code, modified baseline sources or changed analyzer assets. Version probes should produce stable version information; workspace-dependent banners that differ between runs are not comparable evidence.

Earlier draft configurations that generate reports without `tools` must add executable version probes before they can pass.

Report modes:

- `full`: preserve all report diagnostics.
- `changed_lines`: retain mapped diagnostics/coverage on inserted or changed lines. A diagnostic without a mappable file or line cannot silently disappear.
- `new_diagnostics`: rerun the same analyzer on the base commit in another isolated workspace. `baseline` is the output report path in that workspace, usually equal to `path`. Compare stable diagnostic identities and multiplicity, so moving a diagnostic to a different line does not create a new violation. The CLI retains baseline command, snapshot and report evidence; it does not trust an arbitrary old report in the working directory.
- `affected_scope`: the analyzer must provide `affected_files` in its generic diagnostic report. Missing impact information is incomplete validation. Coverage currently uses `full` or `changed_lines`.

```json
{
  "issues": [
    {
      "rule": "dependency-direction",
      "file": "src/orders.rs",
      "line": 12,
      "message": "Orders must not depend on the presentation module",
      "symbol": "orders::create"
    }
  ],
  "affected_files": ["src/orders.rs"]
}
```

File paths must map uniquely to the checked repository. The envelope is generated by the configured analyzer; the CLI supplies execution and snapshot binding. `minimum_tests` and `minimum_coverage` require their corresponding data, rather than treating absent counters as zero or success.

## Coverage inventory and thresholds

Coverage checks require explicit `coverage_paths`, relative to the repository, to define the expected source inventory. In `full` mode every matching source file must appear in the fresh report's inventory or line records. In `changed_lines` mode that requirement applies to matching files with changed/inserted lines. Missing files, duplicate line records after source-path mapping, impossible line numbers and inconsistent counters make the check incomplete.

```yaml
path: target/coverage.info
format: lcov
mode: changed_lines
coverage_paths: ['src/**/*.rs']
minimum_coverage: 90
require_branch_coverage: true
```

`minimum_coverage` defaults to 100 and applies to both line and branch rates. `require_branch_coverage` defaults to true; line-only tools require an explicit policy setting of false. Missing branch evidence cannot become 100% branch coverage. For LCOV, every source section must supply consistent `BRF` and `BRH` summaries to establish branch evidence. Cobertura requires positive, consistent `branches-valid` counts to establish branch measurement; zero also occurs when measurement is disabled. Native `coverage_py` JSON distinguishes measured zero branches using its explicit flag. JaCoCo validates source counters and their summary rollups. See [coverage profiles](coverage.md) for source roots, exclusions, recognized inert XML declarations and real producer verification.

LCOV summaries are checked against detail records. Multiple test sections are merged by line and branch identity, with hits combined without increasing the denominator; duplicates inside a section are malformed. This follows the [LCOV record identities and summary counters](https://github.com/linux-test-project/lcov/blob/master/docs/man/geninfo.rst). Branches must refer to executable line records. Consistent summaries alone do not prove measurement when every branch count is zero: an LCOV branch contract also requires positive branch records. This closes the same zero-counter ambiguity as Cobertura; see the [LLVM acceptance fixture](coverage.md#lcov-zero-counters).

The generic JSON adapter accepts `coverage` line records, `coverage_files` (the analyzer's source inventory, including files with no executable lines), `coverage_roots` (optional explicit roots) and `branch_coverage` (whether branch evidence was collected). A line's optional `excluded` flag removes it from the denominator and requires zero hits/branches; its location must still fit the snapshot. These remain claims from the configured analyzer and require the same fresh execution and policy trust as other reports. Native coverage adapters derive these fields from their reports. LCOV files with no executable lines require explicit `LF:0` evidence.

When a complete inventory shows no selected executable lines or branches, the corresponding percentage is `null`; metadata records the empty executable scope. It is never displayed as measured 100% coverage. Scope selection and exclusions are policy, while executable-line and branch facts come from the coverage tool. [JaCoCo derives these facts from compiled bytecode and debug information](https://www.jacoco.org/jacoco/trunk/doc/counters.html); source-text heuristics cannot substitute for that evidence.
