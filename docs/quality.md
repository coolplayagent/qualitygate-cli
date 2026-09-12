# Rust quality gates

The workflow structure follows `/opt/workspace/relay-knowledge`:

- `.github/workflows/code_quality.yml` preserves its manually triggered Qodana Rust scan. Repository maintainers configure `QODANA_TOKEN` to use that optional workflow.
- `.github/workflows/pr-checks.yml` keeps formatting, compilation, Clippy, unit tests, Rust integration, documentation, architecture, benchmark, coverage (90%), Miri and AddressSanitizer as distinct gates. Windows/macOS tests and Cargo packaging validate CLI portability and distribution.
- Project-specific browser, graph-service and knowledge-map checks from the reference do not apply to this CLI. Documentation and architecture harnesses are Rust tests rather than Python scripts.
- `qualitygate.yaml` runs the regular local checks through the CLI itself; `.pre-commit-config.yaml` exposes stable Rust commands as local hooks. Miri and ASan remain separate nightly gates.
- The local policy records Rustfmt, Cargo, Rustc and Clippy versions through actual probes. The execution integration gate covers command input mutation, baseline validity, version failures and durable evidence.
- The separate `maven-project` job runs three live Maven dependency/repair tests with JDK 21: test-dependency pairing, compiled module directions and used-but-undeclared bytecode analysis. Ordinary Rust suites explicitly ignore these network-dependent tests; the dedicated job executes them and fails on missing tools/evidence. See [project verification](projects.md).

Rust is pinned to 1.97.1 by `rust-toolchain.toml`; nightly is required only for Miri and ASan. The local host's `stable` alias is unusable despite the installed pinned toolchain, so local builds use that explicit pin without changing global toolchains.

The independent `python-project` job uses Python 3.12/pip 26.0.1 to run real installation, declaration-retention and pytest repair checks. It runs the explicitly ignored `tests/python.rs` fixture; missing tools or installation evidence fail the job. See [Python project verification](python-projects.md).

```bash
cargo test --locked --lib --bins --all-features
cargo test --locked --test cli --test custom_rules --test merge_request --test execution --test init --test manual --test provenance --test git_trailers --test compatibility --all-features
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
so a rule cannot silently point at an unarchived source or claim an unimplemented
design, analyzer, or benchmark input as an enforced check.

The Rust integration job and local integration check include `tests/init.rs`. Its temporary Cargo project executes a generated candidate, reports a real failed assertion, passes after repair, and rejects a zero-test result. The repository-owned fixture and initialization implementation are Rust.

`tests/manual.rs` signs external review records in Rust, verifies approval/rejection and task bindings, repairs stale approvals after code changes, and executes a temporary Rust command that changes external trust inputs during a check. The gate must reject that changed evidence while retaining the original artifacts.

`tests/provenance.rs` signs complete run histories and verifies mixed human/agent edits, declaration retention, Python/Java entities, file moves, staged snapshots, omitted steps, unauthorized signers, revocation and tampering. A temporary Rust command proves that record mutation and expiry after initial verification prevent gate completion. Adapter tests exercise ambiguous lineage and malformed intermediate syntax.

`tests/git_trailers.rs` creates real Git histories to verify trailer/entity association across unrelated edits, moves, copies, staged/worktree differences, merges and conflict resolution. It rejects ambiguous trailers and shallow ancestry and confirms replacement refs cannot rewrite snapshot evidence. The provenance suite also exercises the combination of signed AI-only scope and commit-level declarations.

`tests/compatibility.rs` exercises paired snapshot builds, archive and tool identity, incomplete analyzer evidence, timeouts, staged selection, policy changes and task-required compatibility. The separate `java-compatibility` job runs `tests/compatibility_live.rs` with JDK 21 and a digest-pinned japicmp distribution. It checks real binary/source API breaks, repair, private-constructor compatibility and missing classpaths; see [compatibility](compatibility.md).
