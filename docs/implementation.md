# Implementation and verification ledger

This ledger preserves the complete v0.2 requirements while implementation proceeds. Pending items are not advertised as supported. No milestone or coverage threshold replaces the full requirement scope.

| Requirement | Implementation and authoritative verification | State |
|---|---|---|
| §1–2 harness boundaries, strict results and exit codes | `domain/gate.rs`, gate truth-table tests and CLI 0/1/2 scenarios | Core verified; delivery audit pending |
| §3.1 source declarations, matching dependencies, naming, parameterization, comments, line endings, commits, imports and module boundaries | Syntax adapters and seven built-ins; `structure_rules_tests.rs` | Partial: dependency and boundary semantics remain |
| §3.2 build commands, compilation, tests, static analysis, coverage, compatibility | Bounded command runner; JUnit, Checkstyle, SpotBugs, PMD, SARIF, LCOV, Cobertura, JaCoCo and generic JSON parsers | Partial: additional invariant and compatibility verification remains |
| §3.3–3.4 profiles, task contracts, repair feedback and manual evidence | `config/plan.rs`, actual command execution and task/profile CLI scenarios | Partial: manual acceptance and stronger trust validation remain |
| §3.5 commit, staged, worktree and path snapshots; all increment modes | Snapshot/baseline CLI fixtures; report-gate tests for changed lines, multiplicity, renamed files and affected unmodified sources; coverage omission/branch repair CLI loop | Core modes verified; project impact and full evidence lifecycle audit remain |
| §4–5 syntax/project/tool adapter ownership and multiple languages | Java, Python, TypeScript, Go, Rust and Shell syntax fixtures; Rust architecture test | Syntax verified; project semantics and complete boundary audit pending |
| §6.1–6.2 rule packages and custom DSL | Embedded core/shared/Java/Python packages; snapshot-bound custom loading, strict validation, source/definition evidence and `tests/custom_rules.rs` | Syntax/Git/file DSL implemented; dependency and external provenance capabilities remain |
| §6.3 applicability, execution and completeness | Pure domain tests; CLI 0/1/2 assertions, including missing required tool and stale report | Core verified; final audit pending |
| §6.4 declaration scopes, trailers and external source evidence | Explicit marker fields and missing-record tests | Partial: commit-to-entity association and external provenance remain |
| §6.5 init, repeat safety, discovery and candidate configuration | Selected configuration path; custom catalog list/enable/show; effective defaults; JSON/table/Markdown integration scenarios | Core commands verified; expanded project discovery remains |
| §6.6 source mapping, trusted policy and changed verification assets | Source digest checker, caller-supplied policy comparison and policy-tampering CLI fixture | Partial: comprehensive trust and verification-asset coverage remains |
| §7 all CLI commands and MR providers | Help, parser and isolated remote/provider tests | Pending |
| §8 JSON/table/Markdown, fingerprints, executions and snapshot binding | JSON/table/Markdown check reports, durable logs and report artifacts, partial timeout logs, source/workspace revalidation | Partial: tool/environment versions and complete lifecycle audit remain |
| §9.1 acceptance scenarios | Requirement-specific unit/integration suites | Pending |
| §9.2 pilot measurement and actual repair loop | Reproducible pilot protocol, run evidence, agreed acceptance measurements | Pending |
| §10 rollout and stable extensibility | Complete implementation and compatibility fixtures | Pending |
| Reference-equivalent quality YAML | `code_quality.yml`, Rust PR matrix, ≥90% coverage gate, Miri/ASan, native Windows/macOS, packaging; local `qualitygate.yaml` | Files implemented; complete gate execution pending |

Reference inspected: `/opt/workspace/relay-knowledge`, including `Cargo.toml`, `.github/workflows/code_quality.yml`, `.github/workflows/pr-checks.yml`, and its architecture constraints. The reference has unrelated local changes and is read-only for this task. Its available committed graph is pinned to `a6a0c8a9ed7518534e1fd0e49f079b73d179764b`; its newer indexing task is retrying, so current workflow details are checked directly against files.

## Verification checkpoint

The local verification checkpoint on 2026-09-08 passed 52 library tests, nine core CLI integration tests, 13 custom-rule CLI integration tests, two documentation/architecture tests and one deterministic benchmark (77 tests total). Formatting, `cargo check --locked --all-targets --all-features`, and `cargo clippy --locked --all-targets --all-features -- -D warnings` also passed.

`cargo llvm-cov --locked --all-targets --all-features --summary-only --fail-under-lines 90` passed at 91.93% Rust line coverage (4,227 lines, 341 missed), improving on the earlier 82.06% checkpoint. Coverage includes the custom DSL, source/definition trust checks, report-scope invariants, coverage inventory/branch handling and command precondition/timeout scenarios. These results verify the implemented scope above; the remaining requirements and CI-only gates still need verification.

Self-hosted `cargo run --locked -- check --worktree --profile full --output-dir target/self-check --format json` completed all seven configured checks with no violations. Its immutable input digest was `sha256:4b0ae7231a61278f63886c1233aa1bf4419b291b853e0a535ae20c99ab312d32`, based on commit `e805626d33c438cd5e5157abbcd619f8be567041` plus the then-current worktree. Local evidence is in `target/self-check/run-wDL9nA/report.json` and adjacent logs. That checkpoint preceded the final rule-revision/protocol separation and permission-preserving configuration write, which were subsequently validated by the complete 77-test coverage run.

`cargo package --locked --allow-dirty` also passed, including Cargo's package compilation verification. The archive contains all nine embedded rule YAML files. These are local checkpoint results; Miri, ASan and native Windows/macOS jobs still require authoritative CI or platform execution.

The initial remote workflow failed GitHub's YAML validation before any job started: the unquoted Miri command ended with `domain::`. The command is now quoted, and `documentation_yaml_contracts_are_parseable` checks repository YAML files locally. This adds one documentation test beyond the 77-test coverage checkpoint; remote platform gate outcomes remain pending.

## Remaining implementation and audit work

- Complete the semantic and external-provenance capabilities used by custom DSL assertions; audit protocol migration against real second-ecosystem evidence.
- Implement project/semantic dependency checks, annotation dependency pairing, module boundary rules, and interface compatibility integrations.
- Complete trusted external manual acceptance and agent-run provenance, including the association between a declaration and the actual changed entity/commit.
- Add the `--mr` command with bounded network access, provider metadata and merge-base resolution; validate against controlled provider fixtures.
- Audit trusted strategy updates and verification asset coverage. Source hashing now rejects duplicate/code-only headings; catalog/settings/engine versions participate in rule digests; candidate commands respect selected configuration paths and output formats.
- Finish coverage-tool compatibility and SARIF/baseline execution validation. Explicit coverage inventories now reject omitted source files, duplicate mapped lines and missing branch evidence; LCOV section counters and branch identity merges are tested, and empty executable scopes use null rates.
- Complete entity-change auditing for changed names, cross-file comments and declaration retention. Multiset tests now prove moves/copies, Java overloads and annotation removal; mixed-language/module boundaries still require project facts.
- Keep CPU parsing and filesystem-heavy validation behind explicit asynchronous worker boundaries. Add tool/environment provenance, and prove command input mutation invalidates each dependent result.
- Strengthen the architecture harness from basic purity checks to a real acyclic module/dependency gate; validate Markdown anchors and YAML contracts.
- Maintain the unchanged ≥90% coverage threshold as implementation expands; run self-hosted quality checks, packaging, Miri, ASan and native-platform verification where available.
- Run and record a reproducible real-repository pilot and its measurement protocol. Do not substitute synthetic successes for unmeasured human-review savings.
