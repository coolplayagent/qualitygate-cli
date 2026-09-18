# Rust quality gates

The workflow structure follows `/opt/workspace/relay-knowledge`:

- `.github/workflows/code_quality.yml` preserves its manually triggered Qodana Rust scan. Repository maintainers configure `QODANA_TOKEN` to use that optional workflow.
- `.github/workflows/pr-checks.yml` keeps formatting, compilation, Clippy, unit tests, Rust integration, documentation, architecture, benchmark, coverage (90%), Miri and AddressSanitizer as distinct gates. Windows/macOS tests and Cargo packaging validate CLI portability and distribution.
- Miri runs the complete pure-domain test inventory in four disjoint shards: evolution, pilot base, pilot versions, and the remaining domain tests. The 2026-09-18 inventory check selected 2 + 12 + 21 + 27 = 62 tests, the same 62 selected by `domain::`. This split keeps the long interpreted evidence-boundary test inside its own job without omitting coverage.
- Minimal `selfcheck` is a separate PR gate; `.github/workflows/selfcheck.yml` runs the full fixture corpus nightly on Linux, Windows and macOS and preserves JSON diagnostics. See [fixture regression](selfcheck.md).
- The 764-fixture corpus includes 85 policy-evolution cases. `tests/selfcheck.rs` requires unchanged goldens to detect weakened commit and policy evaluators, actual timeout/tool-error evidence, serial/parallel agreement, and isolated native Git setup.
- Project-specific browser, graph-service and knowledge-map checks from the reference do not apply to this CLI. Documentation and architecture harnesses are Rust tests rather than Python scripts.
- `qualitygate.yaml` runs the regular local checks through the CLI itself; `.pre-commit-config.yaml` exposes stable Rust commands as local hooks. Miri and ASan remain separate nightly gates.
- The local policy records Rustfmt, Cargo, Rustc and Clippy versions through actual probes. The execution integration gate covers command input mutation, baseline validity, version failures and durable evidence.
- The separate `maven-project` job runs three live Maven dependency/repair tests with JDK 21: test-dependency pairing, compiled module directions and used-but-undeclared bytecode analysis. Ordinary Rust suites explicitly ignore these network-dependent tests; the dedicated job executes them and fails on missing tools/evidence. See [project verification](projects.md).

When a Maven fixture returns an unexpected exit code, its harness prints up to
eight fixture-local producer artifacts, at most 16 KiB each, before temporary
cleanup. This keeps the actual tool failure visible in CI rather than only
recording paths to deleted logs. The expected exit codes and assertions remain
unchanged; a retry cannot substitute for diagnosing a repeated failure.

Rust is pinned to 1.97.1 by `rust-toolchain.toml`; nightly is required only for Miri and ASan. The local host's `stable` alias is unusable despite the installed pinned toolchain, so local builds use that explicit pin without changing global toolchains.

The independent `python-project` job uses Python 3.12/pip 26.0.1 to run real installation, declaration-retention and pytest repair checks. It runs the explicitly ignored `tests/python.rs` fixture; missing tools or installation evidence fail the job. See [Python project verification](python-projects.md).

```bash
cargo test --locked --lib --bins --all-features
cargo test --locked --test bazel --test cli --test custom_rules --test file_contracts --test ratchet --test rule_authoring --test rule_management --test policy_categories --test policy_candidates --test merge_request --test execution --test init --test manual --test provenance --test git_trailers --test compatibility --test sarif --test coverage --test source_reviews --test policy --all-features
cargo test --locked --test quality --all-features
cargo test --locked --test benchmarks --all-features
cargo llvm-cov --locked --all-targets --all-features --fail-under-lines 90
```

Workflow presence is not evidence that a gate passed. Actual results, coverage measurements and remaining work are recorded in [implementation.md](implementation.md).

The [architecture and documentation contract](architecture.md) describes the production owner graph, source-digest evidence, Markdown anchor validation and their limits. Architecture and documentation continue to run as distinct CI jobs and together in the local `quality` check.

The [external standards archive](../knowledge/best-practices/engineering-standards/README.md)
contains dated summaries and official links for the eight requested organizations,
supplemented by language-ecosystem guidance where it is needed for coverage.
Built-in rule catalog loading validates every `standard_refs` and
`lifecycle_inputs` entry against the machine-readable
[registry and lifecycle matrix](../knowledge/best-practices/engineering-standards/lifecycle-rule-matrix.yaml),
so a rule cannot silently point at an unarchived source, attach an undeclared
normalized control to a reviewed source, or claim an unimplemented design,
analyzer, or benchmark input as an enforced check. Registry schema v5 also
rejects incomplete source provenance, non-HTTPS links,
unknown archive classifications, taxonomy drift, missing declared organization,
language, lifecycle, or concern coverage, absent direct lifecycle paths for
those declared dimensions, attempts to narrow the embedded eight-company/full
lifecycle research scope, and status/outcome mismatches;
the native configuration tests cover these archive-contract failures separately
from rule execution.

The [skill-over-CLI package](skill-package.md) is also a versioned quality
contract. Its Rust quality test checks that the skill metadata follows the Cargo
version, preserves the policy/evidence safety boundary, and names the release
assets; the tag workflow then validates the generated archive before any public
publication.

`tests/quality/site.rs` checks the static Pages entrypoint, current release
version, local assets, repository documentation links and deployment workflow.
The online release archive and Pages URL still require post-deployment checks.
The Agent-loop no-progress fixture uses a stdin-consuming Git command so a
fast process exit cannot turn prompt delivery into a platform-dependent broken
pipe; its stop-reason and attempt-count assertions remain unchanged.

The Rust integration job and local integration check include `tests/init.rs`. Its temporary Cargo project executes a generated candidate, reports a real failed assertion, passes after repair, and rejects a zero-test result. The repository-owned fixture and initialization implementation are Rust.

`tests/pilot_baseline.rs` additionally validates the versioned Rust bug-fix and
refactor templates, actual assertions, zero-test rejection, compilation gaps and
selected-task protection. It preserves reports and tool logs under
`target/pilot-phase-a`. `tests/pilot_codex_live.rs` is an explicitly ignored,
billable connectivity probe, separate from stable tests. See the
[phase-A record](pilot-phase-a.md) for actual runs and their limits.

`tests/manual.rs` signs external review records in Rust, verifies approval/rejection and task bindings, repairs stale approvals after code changes, and executes a temporary Rust command that changes external trust inputs during a check. The gate must reject that changed evidence while retaining the original artifacts.

`tests/provenance.rs` signs complete run histories and verifies mixed human/agent edits, declaration retention, Python/Java entities, file moves, staged snapshots, omitted steps, unauthorized signers, revocation and tampering. A temporary Rust command proves that record mutation and expiry after initial verification prevent gate completion. Adapter tests exercise ambiguous lineage and malformed intermediate syntax.

`tests/git_trailers.rs` creates real Git histories to verify trailer/entity association across unrelated edits, moves, copies, staged/worktree differences, merges and conflict resolution. It rejects ambiguous trailers and shallow ancestry and confirms replacement refs cannot rewrite snapshot evidence. The provenance suite also exercises the combination of signed AI-only scope and commit-level declarations.

`tests/compatibility.rs` exercises paired snapshot builds, archive and tool identity, incomplete analyzer evidence, timeouts, staged selection, policy changes and task-required compatibility. The separate `java-compatibility` job runs `tests/compatibility_live.rs` with JDK 21 and a digest-pinned japicmp distribution. It checks real binary/source API breaks, repair, private-constructor compatibility and missing classpaths; see [compatibility](compatibility.md).

`tests/sarif.rs` covers indexed paths, multiple locations and ranges, stable baseline comparison, suppressed findings and incomplete analysis. The independent `sarif-project` job installs clippy-sarif 0.8.0 and runs `tests/sarif_live.rs` with the pinned Clippy toolchain, proving actual warning repair and rejection of compiler failure. See [SARIF](sarif.md) for the supported profile and evidence limits.

`tests/coverage.rs` checks fresh native JSON and Cobertura reports, source-root ambiguity, staged selection, policy changes, timeouts, input mutation and original artifacts. The separate `coverage-producers` job runs `tests/coverage_live.rs` using JDK 21, digest-pinned JaCoCo 0.8.15, Python 3.12, coverage.py 7.10.7 and pytest 8.4.2. These two live tests exercise line/branch repair, missing debug/source evidence, XML/JSON equivalence, exclusions, measured zero branches and real test/compilation failures. See [coverage reports](coverage.md).

The same job separately runs `tests/coverage_llvm_live.rs` with the pinned Rust/LLVM tools. It proves that zero branch counters in real LLVM LCOV output cannot establish branch instrumentation, while the measured line coverage can satisfy an explicitly line-only policy.

`tests/source_reviews.rs` covers missing/stale source reviews, hash-only refresh attempts, rule changes, optional checks, staged inputs and caller-selected policy versions. Pure domain review-contract tests participate in Miri; native tests remain in the separate ASan gate. Markdown source mapping uses CommonMark recognition, with regressions for indented pseudo-closing fences, HTML comments and code-only headings. Strict YAML loading rejects duplicate mappings before typed maps can overwrite entries.

`tests/policy.rs` preserves selected task plans and acceptance conditions after candidate deletion, corruption and executable-mode changes. It verifies staged task isolation and policy commit evidence, then moves an actual temporary branch and executes a diagnostic's pinned recheck. JSON, table and Markdown retain the selected task's identity and descriptions. See [task plans and policy selection](tasks.md).

The separate unit gate checks directly constructed task contracts at the reusable `Plan::build` API, so YAML loading is not a prerequisite for semantic validation. These configuration tests run with native unit tests; Miri remains restricted to the pure domain.

The `feedback` and `agent_loop` integration targets cover phase-B presentation
and external retry contracts. `pilot_baseline` also executes a real Git merge
whose individually passing branches regress when combined. The explicitly
ignored `pilot_repair_live` target makes billable model calls; see
[phase B](pilot-phase-b.md) for actual records and remaining pilot limits.

The `case_provenance`, `pilot_summary` and `pilot_templates` targets cover phase-C
lineage, immutable promotion inputs, assignment denominators, full-report
bindings, malformed/missing evidence and the remaining task templates. Pure
metric and zero-denominator decisions remain in the native unit target. See
[phase C](pilot-phase-c.md); actual trial observations remain separate.

The `pilot_seal` target and native pilot tests cover phase-D pre-observation
integrity: complete governance fields, real-task ground truth, balanced
model/workflow matrices, late-seal rejection, permitted actual-model disclosure
and protected-plan mutation. See [phase D](pilot-phase-d.md). Digest agreement
does not authenticate the caller or start the real seven-day observation.

The `pilot_authorization` target and adapter unit tests cover phase-E owner
authentication: exact sealed subject and repository binding, external trust
inputs, key scope, human identity, validity window, expiry and revocation.
See [phase E](pilot-phase-e.md). Start authorization does not supply trusted time
or final reviewer acceptance.

The `pilot_acceptance` target separates phase-F threshold and external-review
behavior. Native domain tests cover failed/unknown aggregation; adapter tests
cover reviewer identity, decision eligibility, timing and revocation; CLI tests
bind complete reports and reject changed observation evidence. See
[phase F](pilot-phase-f.md).

Phase G extends the native pilot budget and acceptance tests and the same
`pilot_acceptance` CLI target. It covers v2 seal drift, all-attempt and human
costs, unknown versus exceeded cap, and tenth-check signed refusal. See
[phase G](pilot-phase-g.md); v1 nine-check records remain distinct.

Phase H extends native pilot validation and the `pilot_seal` CLI target with
v3 declared task strata, distinct-input counting, duplicate-task refusal and
seal-drift checks. See [phase H](pilot-phase-h.md); v1/v2 records retain their
serialized plan and threshold semantics.

Phase I extends the same domain and CLI targets with v4 source records,
bounded archive-byte checks and source revalidation on each pilot operation.
The integration test covers changed, escaping and symlinked files; see
[phase I](pilot-phase-i.md). v1/v2/v3 records remain compatible.

Phase J extends native pilot validation and the `pilot_seal` CLI target with
v5 sealed alternating order, stratum balance and observed start-sequence audit.
See [phase J](pilot-phase-j.md); v1–v4 records retain their serialized plans.

Phase K extends native pilot validation and the `pilot_seal` CLI target with
v6 initial full-report binding, bounded report rereads and deterministic
attempt-budget/no-progress audit. See [phase K](pilot-phase-k.md); v1–v5
records retain their serialized plans.

Phase L extends native pilot validation and the `pilot_seal` CLI target with
v7 bounded model-capture files, sealed request identity, explicit unknown
actual models and summary audit. See [phase L](pilot-phase-l.md); v1–v6
records retain their serialized plans and acceptance semantics.

Phase M extends native pilot validation and the `pilot_seal` CLI target with
v8 per-attempt model captures, bounded rereads and reported route-drift audit.
See [phase M](pilot-phase-m.md); v1–v7 plans and acceptance semantics remain
versioned.

Phase N extends the native pilot validation and `pilot_seal` CLI target with
v9 bounded execution receipts, duration binding and archived start-time order
audit. See [phase N](pilot-phase-n.md); v1–v8 plans remain versioned.

Phase O adds v10 unpriced pilot sealing and eight nonfinancial acceptance
checks. See [phase O](pilot-phase-o.md); v1–v9 financial semantics remain versioned.
