# qualitygate-cli

A Rust CLI for repository policy and task acceptance checks. Results distinguish violations from incomplete execution and bind evidence to the code and policy that were checked.

The implementation follows [REQUIREMENTS.md](REQUIREMENTS.md). Implementation and verification progress is tracked in [docs/implementation.md](docs/implementation.md); a requirement is complete only when its behavior and tests exist.

## Development

```bash
cargo build --locked
cargo fmt --all -- --check
cargo check --all-targets --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
```

The CLI, checks and test harnesses are implemented in Rust. Git is required for repository snapshots; configured build and test commands require their own project toolchains.

See [quality gates](docs/quality.md) for CI, coverage and deep verification.
See [tool reports](docs/reports.md) for test counts, coverage, analyzer output and incremental comparison.
See [built-in rules](docs/rules.md) for language adapters and team-specific configuration.

## Current command surface

```bash
cargo run -- init
cargo run -- check --worktree --profile quick --format json
cargo run -- check --staged --format markdown
cargo run -- check --diff HEAD~1..HEAD --format json
cargo run -- rules list
cargo run -- rules enable commit-message
cargo run -- config --show
```

`check` returns 0 for a complete passing gate, 1 for blocking violations, and 2 for incomplete validation. `quick` and `--path` cover only their selected scope and cannot establish delivery readiness. Generated logs and reports live under `QUALITYGATE_HOME` (default: a qualitygate directory in the OS temporary directory); `--output-dir` overrides the evidence location. Commands run against a disposable materialization of the selected snapshot.

Current explicit input limits are 20,000 files, 2 MiB per file, 12 MiB total snapshot contents, and 16 MiB per captured process stream. Exceeding a limit reports incomplete validation. Git symlinks, submodules and unresolved index conflicts also require explicit handling and currently fail closed.
