# Development and quality gates

[简体中文](../../zh/04-contributor-guide/01-development-and-quality-gates.md) · [Volume index](README.md)

Repository-owned implementation is Rust. Tests may invoke Git and a configured
fixture project's own tools at the documented CLI boundary. Use isolated
temporary Git repositories; never change the reference repository, user Git
configuration, or unrelated runtime state.

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
