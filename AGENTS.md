# Repository guidelines

Implement the CLI, reusable core, adapters, test harnesses and documentation checks in Rust. Reference architecture: `/opt/workspace/relay-knowledge`. Its application-specific graph, service and browser requirements do not apply to this CLI.

## Ownership

- `src/qualitygate/domain`: serializable contracts and pure gate decisions; no process, filesystem or network I/O.
- `config`: strict policy and task-contract loading, validation and planning.
- `snapshot`: Git snapshots, change mapping and immutable execution inputs.
- `adapters`: language structures and external report normalization.
- `runner`: bounded asynchronous command execution and evidence collection.
- `application`: orchestrates the check and repair-feedback workflow.
- `interfaces`: CLI parsing and output rendering.
- `env`, `paths`, `net`: environment reads, confined paths, and network ownership respectively.

Keep dependencies acyclic. Extract shared contracts into the domain rather than making adapters depend on the CLI. Isolate blocking filesystem or parsing work from asynchronous orchestration. Bound process output, file sizes, concurrency and execution time. Do not introduce unsafe code, placeholder implementations or silent successful fallbacks.

The executable top-level owner contract and supported source forms are documented in [docs/architecture.md](docs/architecture.md). Run the architecture gate after changing module ownership or imports; keep its source-digest and file/line evidence reviewable.

CodeSpec map: codespec/codespec-map.yaml
Knowledge map: knowledge/knowledge-map.yaml

## Quality contract

- Run `cargo fmt --all -- --check`, `cargo check --all-targets --all-features`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-targets --all-features`.
- Keep unit and integration gates distinct. Cover failure, timeout, incomplete evidence, policy changes, and snapshot mismatches.
- Maintain at least 90% Rust line coverage with `cargo llvm-cov --all-targets --all-features --fail-under-lines 90`.
- Miri exercises the pure domain; ASan exercises native unit tests. Keep these separate from the stable local checks.
- Keep authored source, tests, docs and workflow files within 1000 lines. The generated `Cargo.lock` is exempt.
- Every behavior change includes the relevant documentation and requirement-to-test evidence update.
- Never change a failing gate, required check or test merely to claim completion. Reports must distinguish violations from incomplete execution.

## Development

Use isolated temporary Git repositories in tests. Do not modify the reference repository, user Git configuration or unrelated runtime state. Repository-owned code is Rust; invoking Git or a configured project's existing build tools is part of the documented CLI boundary. Tree-sitter grammars, when used, are third-party native dependencies.
