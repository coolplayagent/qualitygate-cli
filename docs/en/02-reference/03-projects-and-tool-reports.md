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

## Agents as Tool Builders

Choose the output from the rule's evidence needs. A project-owned Python, Node.js, or Rust checker can publish a supported diagnostic report for a single check. When several rules reuse one measurement, a producer can publish a supported versioned tool-facts collection. Register its invocation in `checks[].argv`, its version probe and source inputs in `checks[].tools`, and its required status in the project policy. Use independent fixtures to verify that a replacement tool preserves the meaning and scope of the evidence. [ASan driver and policy example](../../../templates/asan/README.md)

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

## Project deprecation checks

The [language policies](../../../skills/qualitygate-cli/references/deprecation-policies.md)
cover Java, Rust, C, C++, Go, TypeScript, JavaScript, Python and selected Bash
syntax. Each example runs the language's analyzer on project-selected inputs
and applies project diagnostic mode. Analyzer coverage depends on its symbol
resolution and the selected tool, targets and dependency inventory.

### Java deprecation and forbidden annotations

The [JDK 17 and 21 example](../../../skills/qualitygate-cli/references/java-deprecation.yaml)
blocks ordinary deprecation and removal warnings. The
[JDK 8 example](../../../skills/qualitygate-cli/references/java-deprecation-jdk8.yaml)
checks ordinary deprecation, the category available in that compiler.
Collect detailed English javac output in a fresh UTF-8 file with `-Xstdout`.
Enable the example's lint categories, set the warning cap high, and let Qualitygate
apply blocking severity after compilation. Use the project's actual source
list, classpath, module path, processor configuration and release. Source
paths must stay beneath the command directory without parent segments.
Java's warning exemptions and the selected inputs set coverage.

The [PMD example](../../../skills/qualitygate-cli/references/java-forbidden-annotations.yaml)
checks annotation applications independently. Copy the
[ruleset](../../../skills/qualitygate-cli/references/java-forbidden-annotations.xml)
and configure forbidden annotation types by fully qualified name. Run it after
a successful build against the same source list with all needed types on
its auxiliary classpath. The
`pmd_with_suppressed` format includes records emitted by `--show-suppressed`
as file-level findings. Both examples use project report mode so an unchanged
use site can block delivery.

## Typed tool observations

Commands can declare one `tool_facts` output instead of diagnostic reports or
project reports. Its closed contract specifies `adapter`, `schema`, `path`,
`from_stdout`, `tool_id`, `tool_version`, `scope`, and the exact source `paths`.
Tool-facts protocol v1 fixes its inventory universe to Rust `function_item`
nodes with a body, using `crate_root` and `crate_name`. Closures, functions local
to blocks and functions inside a module named `tests` are outside that universe.
Ambiguous identities and unsupported generic/scoped impl self types reject the
collection; they are not silently excluded. Non-Rust input is unsupported. Project,
optional target and configuration identities are explicit and never inferred
from a Bazel cache or workspace scan. This does not introduce a Cargo workspace.

`adapter: tool_facts` accepts the exported `schema tool-facts-v1` JSON protocol.
Metric outputs declare every expected metric, unit and measurement definition;
every inventoried function must supply each metric. Clone outputs declare their
measurement and acquisition filters. The report must reproduce the independently
checked inventory. An optional `context_path` receives a bounded JSON input with
`producer`, `snapshot_digest` and `spec` before execution; configure the tool's
arguments to read that file. It cannot overwrite snapshot inputs or outputs,
and modifying it invalidates the producer. Declared identities are checked
against the actual command, probes and snapshot, then raw bytes are bound by
their retained artifact digest.

The native adapters accept crapper commit
`c757ec79a6c15dc815238bbd9c2f4601eb54d3cc` and dryer commit
`66ff6d21a42c04afcad89c78a80066176d1294b0`. Installation is an explicit caller
step. Declare executable version probes and inputs for the upstream source,
interpreter and dependency lock; a source commit alone does not pin Python or
its parsing dependencies. Native commands run from the repository root with
each declared source file as an explicit argument. Unsupported invocation flags
are rejected before the tool runs.

Native execution is currently verified for Linux x86_64 with
`tree-sitter-language-pack` 1.25.2. Declare a `tools` probe with ID
`tree-sitter-language-pack` using the analyzer's interpreter; its output must be
exactly `1.25.2`. The `native_runtime` output field declares `pack_version`,
`parser_path` in the selected snapshot, and `parser_digest`. This profile accepts
only the official Rust parser digest
`sha256:43b4551eedd58c54397f9571cacdefdac2ec077eaceb00a43977563c605d03f6`.
Retain that parser as an explicit verification input. The gate copies it into a
private cache, overrides all three package cache/library/manifest paths for the
child process, and supplies an invalid local manifest. Missing parsers fail
locally instead of downloading. The cache and retained evidence are rechecked;
other native platforms are explicitly unsupported. Offline normalization and
the bridge remain portable and do not require a native runtime profile.

Crapper requires `--use-existing-coverage` and an explicit `coverage_producer`
dependency that successfully produced validated full LCOV observations. Only
the verified LCOV files may be discoverable under `target/coverage` and
`coverage`; HTML reports that would override them are rejected. File hashes are
checked before and after execution. A recorded zero remains zero; missing
function coverage is incomplete. Coverage and CRAP retain upstream four-place
rounding in their measurement definitions. An explicitly configured threshold
may return the declared findings exit without failing complete fact collection.

Dryer requires explicit `--threshold`, `--min-lines`, and `--min-nodes` matching
the acquisition contract; stdout additionally needs `--edn`. Both clone endpoints
and node counts are retained, with the upstream twelve-place score definition.
A source requesting similarity floor 0.80 cannot consume collection at 0.82.
Dryer filters unrounded scores before serializing twelve decimal places. For
this pinned measurement, a consumer floor `t` requires acquisition at zero or
at/below `ceil(t * 10^12) / 10^12 - 0.5 * 10^-12`, as well as ordinary scope
containment. This exact rounding-preimage check prevents omitted pairs that
round upward onto the consumer boundary; it does not change numeric comparison
or introduce an epsilon. For a source floor of `0.82`, declare and execute
acquisition at `0.819999999999`, or collect at zero. Acquisition at `0.82` cannot
support that same serialized source floor. Generic envelopes claiming the same
measurement obey the same guard. The [native validation record](../../../codespec/test/tool-facts-native-validation.md)
includes an observed counterexample and the separate producer-only smoke run.
Requested size restrictions must also be covered by acquisition. The supported
entity universe excludes closures and test-module functions; these exclusions
are not proof of absence outside that universe.

Facts are published internally as typed values only after complete collection.
High metric values do not fail the producer. V4 rules apply independent policy
thresholds, explicitly depend on their producer, and reuse a producer once per
run. Final input, executable or retained-artifact invalidation blocks dependent
assertions before the gate is aggregated. Existing lint report semantics remain
unchanged. Start native tool policies as warning/shadow candidates and use the
existing adoption workflow before blocking delivery. Mutator strict acceptance,
model checking and GUI integration are outside this first-stage implementation.

`qualitygate-tool-bridge --format dryer --input captured.edn --context context.json`
uses the same normalization library without running tools. Its context uses the
`NormalizeContext` contract and its facts always have `complete: false` because
offline conversion cannot authenticate execution. Build/install inventory and
the explicit binary-archive exclusions are documented in
[development and quality gates](../04-contributor-guide/01-development-and-quality-gates.md).

## Compatibility, Bazel, and incremental meaning

Java compatibility checks build paired snapshots and use configured binary and
source compatibility tooling. Failure to build either side, resolve the API
surface, or run the selected tool is incomplete.

Bazel is an additional reproducible build surface using Bzlmod and Cargo-aligned
dependencies; it does not replace Cargo's required quality gates. Cache state,
lockfile behavior, platform, and selected targets belong in evidence.

Incremental modes declare their meaning: `new_diagnostics`, `changed_lines`,
`affected_scope`, `ratchet`, `full`, or `project`. A changed file does not make every historical
finding new. When a comparable baseline is unavailable, a required ratchet is
incomplete unless trusted policy explicitly allows a disclosed full fallback.

Choose `mode: project` for diagnostic reports that must block on any finding in
the configured producer scope, including an unchanged caller affected by a
dependency upgrade. This mode has no baseline and includes historical findings.
All reported locations still resolve against captured source, and path feedback
still limits the selected locations. Configure the command to analyze the
intended targets and inputs. Policies can require `reports.project-diagnostics.v1`.

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
