# Rust quality gates

The workflow structure follows `/opt/workspace/relay-knowledge`:

- `.github/workflows/code_quality.yml` preserves its manually triggered Qodana Rust scan. Repository maintainers configure `QODANA_TOKEN` to use that optional workflow.
- `.github/workflows/pr-checks.yml` keeps formatting, compilation, Clippy, unit tests, Rust integration, documentation, architecture, benchmark, coverage (90%), Miri and AddressSanitizer as distinct gates. Windows/macOS tests and Cargo packaging validate CLI portability and distribution.
- Project-specific browser, graph-service and knowledge-map checks from the reference do not apply to this CLI. Documentation and architecture harnesses are Rust tests rather than Python scripts.
- `qualitygate.yaml` runs the regular local checks through the CLI itself; `.pre-commit-config.yaml` exposes stable Rust commands as local hooks. Miri and ASan remain separate nightly gates.
- The local policy records Rustfmt, Cargo, Rustc and Clippy versions through actual probes. The execution integration gate covers command input mutation, baseline validity, version failures and durable evidence.
- The separate `maven-project` job runs three live Maven dependency/repair tests with JDK 21: test-dependency pairing, compiled module directions and used-but-undeclared bytecode analysis. Ordinary Rust suites explicitly ignore these network-dependent tests; the dedicated job executes them and fails on missing tools/evidence. See [project verification](projects.md).

Rust is pinned to 1.97.1 by `rust-toolchain.toml`; nightly is required only for Miri and ASan. The local host's `stable` alias is unusable despite the installed pinned toolchain, so local builds use that explicit pin without changing global toolchains.

```bash
cargo test --locked --lib --bins --all-features
cargo test --locked --test cli --test custom_rules --test merge_request --test execution --all-features
cargo test --locked --test quality --all-features
cargo test --locked --test benchmarks --all-features
cargo llvm-cov --locked --all-targets --all-features --fail-under-lines 90
```

Workflow presence is not evidence that a gate passed. Actual results, coverage measurements and remaining work are recorded in [implementation.md](implementation.md).
