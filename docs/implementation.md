# Implementation and verification ledger

This ledger preserves the complete v0.2 requirements while implementation proceeds. Pending items are not advertised as supported. No milestone or coverage threshold replaces the full requirement scope.

| Requirement | Implementation and authoritative verification | State |
|---|---|---|
| §1–2 harness boundaries, strict results and exit codes | `domain/gate.rs`, gate truth-table tests and CLI 0/1/2 scenarios | Core verified; delivery audit pending |
| §3.1 source declarations, matching dependencies, naming, parameterization, comments, line endings, commits, imports and module boundaries | Syntax adapters and seven built-ins; `structure_rules_tests.rs` | Partial: dependency and boundary semantics remain |
| §3.2 build commands, compilation, tests, static analysis, coverage, compatibility | Bounded command runner; JUnit, Checkstyle, SpotBugs, PMD, SARIF, LCOV, Cobertura, JaCoCo and generic JSON parsers | Partial: additional invariant and compatibility verification remains |
| §3.3–3.4 profiles, task contracts, repair feedback and manual evidence | `config/plan.rs`, actual command execution and task/profile CLI scenarios | Partial: manual acceptance and stronger trust validation remain |
| §3.5 commit, staged, worktree and path snapshots; all increment modes | Snapshot tests and independently executed baseline-analysis CLI fixture | Partial: impact/coverage edge cases remain |
| §4–5 syntax/project/tool adapter ownership and multiple languages | Java, Python, TypeScript, Go, Rust and Shell syntax fixtures; Rust architecture test | Syntax verified; project semantics and complete boundary audit pending |
| §6.1–6.2 rule packages and custom DSL | Validated YAML and custom rule execution tests | Pending |
| §6.3 applicability, execution and completeness | Pure domain tests; CLI 0/1/2 assertions, including missing required tool and stale report | Core verified; final audit pending |
| §6.4 declaration scopes, trailers and external source evidence | Explicit marker fields and missing-record tests | Partial: commit-to-entity association and external provenance remain |
| §6.5 init, repeat safety, discovery and candidate configuration | Implemented init/list/enable/config commands; repeat-safety tests | Partial: expanded discovery, custom-rule commands and format parity remain |
| §6.6 source mapping, trusted policy and changed verification assets | Source digest checker, caller-supplied policy comparison and policy-tampering CLI fixture | Partial: comprehensive trust and verification-asset coverage remains |
| §7 all CLI commands and MR providers | Help, parser and isolated remote/provider tests | Pending |
| §8 JSON/table/Markdown, fingerprints, executions and snapshot binding | JSON/table/Markdown check reports, durable logs and report artifacts, partial timeout logs, source/workspace revalidation | Partial: tool/environment versions and complete lifecycle audit remain |
| §9.1 acceptance scenarios | Requirement-specific unit/integration suites | Pending |
| §9.2 pilot measurement and actual repair loop | Reproducible pilot protocol, run evidence, agreed acceptance measurements | Pending |
| §10 rollout and stable extensibility | Complete implementation and compatibility fixtures | Pending |
| Reference-equivalent quality YAML | `code_quality.yml`, Rust PR matrix, ≥90% coverage gate, Miri/ASan, native Windows/macOS, packaging; local `qualitygate.yaml` | Files implemented; complete gate execution pending |

Reference inspected: `/opt/workspace/relay-knowledge`, including `Cargo.toml`, `.github/workflows/code_quality.yml`, `.github/workflows/pr-checks.yml`, and its architecture constraints. The reference has unrelated local changes and is read-only for this task. Its available committed graph is pinned to `a6a0c8a9ed7518534e1fd0e49f079b73d179764b`; its newer indexing task is retrying, so current workflow details are checked directly against files.

## Verification checkpoint

The local verification checkpoint on 2026-09-08 passed 36 library tests, seven CLI integration tests, two documentation/architecture tests and one deterministic benchmark (46 tests total). Formatting, `cargo check --locked --all-targets --all-features`, and `cargo clippy --locked --all-targets --all-features -- -D warnings` also passed.

`cargo llvm-cov --all-targets --all-features --summary-only` measured 82.06% Rust line coverage (2,937 lines, 527 missed). Rechecking that report with `cargo llvm-cov report --summary-only --fail-under-lines 90` failed the unchanged 90% gate. This is an implementation checkpoint, not full requirement acceptance; the remaining requirements and CI-only gates still need verification.

## Remaining implementation and audit work

- Implement custom rule loading/execution and rule packages rather than merely accepting their schema types.
- Implement project/semantic dependency checks, annotation dependency pairing, module boundary rules, and interface compatibility integrations.
- Complete trusted external manual acceptance and agent-run provenance, including the association between a declaration and the actual changed entity/commit.
- Add the `--mr` command with bounded network access, provider metadata and merge-base resolution; validate against controlled provider fixtures.
- Audit source/rule version hashing, trusted strategy updates, verification asset changes, selected configuration paths and complete CLI output-format semantics.
- Verify incremental coverage cannot pass by omitting affected source files, repeated coverage records or changed branches; strengthen SARIF/source-path and baseline execution validation.
- Improve entity-change mapping for method renames, copies, overloaded methods, marker removal and mixed-language/module boundaries.
- Keep CPU parsing and filesystem-heavy validation behind explicit asynchronous worker boundaries. Add tool/environment provenance, and prove command input mutation invalidates each dependent result.
- Strengthen the architecture harness from basic purity checks to a real acyclic module/dependency gate; validate Markdown anchors and YAML contracts.
- Reach the unchanged ≥90% coverage threshold with meaningful error/boundary tests, then run self-hosted quality checks, packaging, Miri, ASan and native-platform verification where available.
- Run and record a reproducible real-repository pilot and its measurement protocol. Do not substitute synthetic successes for unmeasured human-review savings.
