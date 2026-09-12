# Java interface compatibility

Requirement §3.2 can require a comparison of the baseline and current Java APIs. The Rust adapter runs the configured build independently on both immutable snapshots, captures its JAR outputs, and invokes a digest-pinned [japicmp CLI](https://siom79.github.io/japicmp/CliTool.html). Repository policy or a task contract selects whether binary compatibility, source compatibility or both are required.

```yaml
schema_version: 1
checks:
  - id: public-api
    argv: [mvn, --batch-mode, package]
    timeout_seconds: 600
    tools:
      - {id: maven, argv: [mvn, --version]}
      - {id: javac, argv: [javac, -version]}
    compatibility:
      tool: japicmp
      analyzer_jar: /opt/tools/japicmp-0.26.2.jar
      analyzer_sha256: sha256:6c65dc29f205fdf57ea28255b901d817321fc1eda6e56afe266f36979a616466
      java: java
      level: both
      artifacts:
        - {baseline: target/library-1.0.jar, current: target/library-1.1.jar}
      classpath:
        - {baseline: target/dependency.jar, current: target/dependency.jar}
      timeout_seconds: 120
```

The example checksum identifies the upstream [japicmp 0.26.2 distribution with dependencies](https://github.com/siom79/japicmp/releases/download/japicmp-base-0.26.2/japicmp-0.26.2-jar-with-dependencies.jar). Provision and review the analyzer before running the check; the production CLI does not download it. Relative analyzer paths resolve against the selected repository root. Java resolves through the command runner's executable lookup; absolute executable paths are also supported. The adapter checks the analyzer bytes against the trusted checksum and reads its embedded Maven identity/version. This protocol supports 0.26.2; other versions remain incomplete until their adapter compatibility is verified. Version 0.26.2 fixes a [private-constructor compatibility regression](https://siom79.github.io/japicmp/ReleaseNotes.html).

`artifacts` and `classpath` contain normalized paths relative to each materialized repository root, even when the build uses another `cwd`. The build must freshly produce all configured JARs, including dependencies needed to resolve inherited APIs. Each side can use different output names. Pair order determines the analyzer input names; all API archives participate in a single comparison. Classpath archives support resolution and are not independently treated as public API inventories. Include libraries whose own APIs must remain compatible in `artifacts`.

The same build `argv`, required arguments, working directory and tool probes apply to both sides. Each build has its own `timeout_seconds`; the nested deadline applies to the analyzer. The analyzer also has a bounded Java version probe and a 512 MiB heap limit. A build has to exit successfully, preserve its selected inputs and produce complete outputs before comparison starts. Required prerequisite checks must complete, but their generated files are not transferred into either fresh build workspace. The configured build must perform its own prerequisites.

Use `qualitygate check --diff <base>..<head>` for release commits, or select a worktree, staged or MR snapshot as described in [the report contract](reports.md). Compatibility always compares complete configured archives. It does not filter failures to changed source lines. Both builds must have matching executable digests and declared tool versions/inputs; changing a wrapper or analyzer environment requires an explicitly supported migration. The ordinary repository policy and verification-asset checks still apply, including `--policy-ref` validation.

A task acceptance item's `verification` supports the same `compatibility` object, build `argv` and tool probes, with `check_id` in place of the repository check's `id`. A compatibility check cannot also map ordinary reports, project facts, manual evidence or findings exit codes. Its successful build exit code is zero. This keeps the comparison's two source snapshots and generated analyzer inputs unambiguous.

## Results and evidence

A completed comparison reports stable class/member identities and the tool's compatibility-change kind. `level` accepts `binary`, `source` or `both` (default). A source-only incompatibility can pass an explicitly binary-only contract; the full source/binary summary remains visible. Blocking incompatibilities produce exit code 1. Missing tools, build failures, deadlines, invalid JARs, missing classes, filtered/missing XML, mismatched tool inputs and snapshot mutations produce incomplete validation and exit code 2. A timed-out build or analyzer retains `timed_out` status.

`metadata.baseline_build` and `metadata.current_build` retain each actual execution record, tool probes, source snapshot and logs. `metadata.compatibility` identifies the analyzer, contract level, source comparison, captured archive bindings and final class counts. Each archive binding includes its source snapshot digest, configured source path, generated input path and durable artifact digest. The outer execution records the actual Java comparison and the digest of its complete materialized analyzer/JAR inputs. Analyzer bytes, API/classpath JARs, logs and XML are retained for review. Internal build evidence uses an identity namespace unavailable to authored check IDs, so a separate check cannot overwrite its evidence.

The Rust parser verifies the exact old/new paths and a full, unfiltered class inventory, including private and synthetic classes. It cross-checks class summaries with member evidence and rejects duplicate or omitted classes. Missing dependency resolution is never suppressed. Both source builds and captured analyzer inputs undergo mutation checks before and after use. These integrity checks retain the [runner's documented limits](reports.md#execution-and-tool-evidence); they do not create an operating-system sandbox.

## Supported inputs and limits

The current adapter supports ordinary Java class JARs. It rejects multi-release JARs and JPMS module descriptors because they require explicit runtime/module compatibility semantics. Resource behavior, reflective lookups, serialization policy and behavioral equivalence are outside this interface comparison. A successful result does not prove task behavior; declare the relevant tests separately.

Limits are 1–32 API archive pairs, at most 64 classpath pairs, 32 MiB per JAR, 128 MiB total captured analyzer inputs, 50,000 ZIP entries per JAR, 256 MiB declared expanded size per JAR, and 50,000 distinct classes per version. Duplicate classes across API and classpath inputs, unsafe ZIP paths and a completely empty compared class inventory are incomplete. XML is limited to 2 MiB and process streams to the runner's existing 16 MiB limit. Build outputs must not overwrite checked input paths. Archive reads and parsing are bounded; parsing and input digest calculation use blocking workers.

## Requirement-to-test evidence

| Requirement | Rust verification |
|---|---|
| §3.2 actual baseline/current comparison and repair | `tests/compatibility.rs::paired_builds_bind_artifacts_find_breaks_and_pass_after_repair` |
| §3.4 task-required interface preservation | `tests/compatibility.rs::task_contract_can_require_a_complete_compatibility_check` |
| §3.5 and §6.6 selected staged bytes, policy changes and level semantics | `tests/compatibility.rs::comparison_uses_staged_inputs_and_respects_binary_source_policy` |
| §6.3 missing/partial/mismatched evidence, failures and deadlines | `tests/compatibility.rs::incomplete_analyzer_output_timeout_and_input_mutation_never_pass`, `missing_build_outputs_and_source_changes_block_comparison` |
| §8.2 complete artifact bindings and independent internal execution evidence | `tests/compatibility.rs::internal_build_evidence_cannot_collide_with_named_checks`, `classpath_inventory_and_tool_inputs_remain_complete_and_comparable` |
| §5.2 bounded/versioned parser and complete inventory | `adapters/compatibility_tests.rs`, `tests/compatibility.rs::analyzer_digest_archive_inventory_and_configuration_are_required` |
| §9.1 real compiler/analyzer integration | `tests/compatibility_live.rs::real_java_binary_source_and_classpath_compatibility_repair` |

The live test uses actual `javac`, `jar` and japicmp, executes a method-deletion/repair sequence, checks a source-only generic return change, adds a private constructor without losing public construction, and rejects missing dependency classpaths. Its Rust harness can use a pre-provisioned checksum-verified analyzer via `QUALITYGATE_TEST_JAPICMP`, and explicit compiler/archive tools via `QUALITYGATE_TEST_JAVAC`/`QUALITYGATE_TEST_JAR`. Otherwise the live harness downloads the pinned analyzer with a byte limit and deadline. This test runs explicitly in the separate `java-compatibility` CI job; ordinary deterministic suites do not require a JDK or network. Temporary generated Java sources are test project inputs, while repository-owned implementation and harnesses are Rust.

```bash
cargo test --locked --lib adapters::compatibility --all-features
cargo test --locked --test compatibility --all-features
cargo test --locked --test compatibility_live -- --ignored --nocapture
```

These controlled compiler/analyzer fixtures are integration evidence, not the measured real-repository pilot required by §9.2. Verification results and remaining delivery requirements are recorded in [the implementation ledger](implementation.md).
