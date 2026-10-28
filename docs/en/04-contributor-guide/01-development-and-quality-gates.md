# Development and quality gates

[简体中文](../../zh/04-contributor-guide/01-development-and-quality-gates.md) · [Volume index](README.md)

Repository-owned implementation is Rust. Tests may invoke Git and a configured
fixture project's own tools at the documented CLI boundary. Use isolated
temporary Git repositories; never change the reference repository, user Git
configuration, or unrelated runtime state.

## Build from source

The checked-in `rust-toolchain.toml` selects the contributor toolchain. Install
its declared components, then build and inspect the local executable:

```bash
cargo build --locked
cargo run -- --version
```

This is a development build, distinct from the installed release/Skill runtime.

## Required checks

Run the stable local contract:

```bash
cargo fmt --all -- --check
cargo check --all-targets --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo llvm-cov --all-targets --all-features --fail-under-lines 90
```

Unit and integration gates remain distinct. Pure domain logic is additionally
eligible for Miri; native unit tests are exercised separately under ASan. Live
producer tests that need external toolchains or networks never count as though
they ran in the stable local suite.

The repository's `integration` command check and CI's `rust-integration` job
select `cargo test --locked --test '*' --all-features`; CI also retains
`--no-fail-fast`. Cargo expands the quoted target glob, so newly added integration
targets enter the gate automatically. The unit gate keeps `--lib --bins`, and
benchmark, documentation and architecture gates continue reporting separately.
The repository quality test compares the policy's selectors with bounded, offline
`cargo metadata` output and rejects missing targets, unit selectors or test-name
filters. Ignored tests remain excluded from the default run; their dedicated live
producer jobs supply execution evidence.

Native Windows CI partitions the complete Cargo target inventory into three jobs:
`--lib --bins --examples --benches`, `--test '[a-m]*'`, and `--test '[!a-m]*'`.
The complementary, quoted Cargo globs include new integration targets without a
maintained name list. Every job retains `--locked --all-features` and a 30-minute
budget. PowerShell/Git Bash preflights run with the first integration group, which
also builds the native CLI. macOS and local checks retain `--all-targets`.
When changing the partition, compare each target's `cargo test ... -- --list`
inventory against the complete command, including empty targets and ignored
tests; their union must be identical and contain no duplicate targets.
See [Cargo target selection](https://doc.rust-lang.org/cargo/commands/cargo-test.html#target-selection).

Windows dev/test profiles omit debug symbols to reduce PDB generation and linking
costs; debug assertions and overflow checks remain explicitly enabled.
These independent [Cargo profile settings](https://doc.rust-lang.org/cargo/reference/profiles.html)
apply only to those Windows steps. Local defaults, test selection, optimization
levels, Miri and ASan stay unchanged. An interrupted job remains incomplete.

The ESLint acceptance and ClawHub release steps use `actions/setup-node@v7`,
whose action runtime is Node 24, and select Node 24 for their tools. Both set
`package-manager-cache: false` to preserve v4's disabled workflow cache;
dependencies are installed by the temporary fixture or global CLI step.
See the [upstream migration notes](https://github.com/actions/setup-node/blob/v7/README.md).

Architecture, coverage and full-selfcheck evidence use `actions/upload-artifact@v6`,
matching the release workflow's Node 24 runtime. Missing evidence still fails
uploads, including uploads after a failed check. The static Pages workflow uses
`configure-pages@v6`, `upload-pages-artifact@v5` and `deploy-pages@v5`;
its upload keeps `path: site` and explicitly excludes hidden files. Validate
the site/package contracts locally and inspect the actual artifact and Pages
workflow results separately; local tests do not prove remote upload or deployment.

Every behavior change updates its relevant documentation and requirement-to-test
evidence. Cover pass, violation, incomplete execution, timeout, policy change,
and snapshot mismatch where applicable. Never weaken a failing check or its test
to claim completion.

All authored source, tests, documentation, and workflow files must be at most
1,000 lines and 2 MiB; generated lockfiles are exempt where documented.
Markdown links, anchors, references, HTML links, and fenced blocks are parsed by
the Rust documentation gate. The bilingual book also enforces path parity,
language switches, complete indexes, and a clean `docs/` root.

Run the architecture test after owner or import changes. Run package and site
tests after Skill, release, version, or documentation-entry changes. Bazel is a
separate supported build surface; it does not replace the Cargo commands above.


## Maintain the documentation

The book is ordinary Markdown; no generator is required. Keep English and Chinese
paths, meanings, language switches and chapter navigation aligned. Tutorials own
the task sequence, references own exact contracts, and architecture owns rationale.
Link to the primary explanation instead of copying it. Remove completed roadmap
language, but retain supported compatibility and evidence boundaries.

```bash
cargo test --test quality --all-features
```

This checks documentation, architecture, site entries and packaged Skill contracts.
It complements the stable checks above. For command examples, use isolated Git
repositories and verify the selected runtime; never change user Git configuration.

## Requirements and repository navigation

[CodeSpec requirements](../../../codespec/requirements/qualitygate-cli.md) are the
normative product contract; the [evidence index](03-requirements-to-test-evidence.md)
maps it to tests. Historical implementation records stay in
[CodeSpec design](../../../codespec/design/README.md).
[CodeSpec Map](../../../codespec/codespec-map.yaml) and
[Knowledge Map](../../../knowledge/knowledge-map.yaml) govern repository navigation.
The project is distributed under the MIT license.

---

[Previous: Rule capabilities and Agent behavior contracts](../03-architecture/07-rule-capabilities-and-agent-contracts.md) · [Contents](../README.md) · [Next: Selfcheck and fixtures](02-selfcheck-and-fixtures.md)
