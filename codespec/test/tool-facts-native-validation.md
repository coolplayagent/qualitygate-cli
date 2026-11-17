# Native tool-facts validation

Observed on 2026-11-17, Linux x86_64, Python 3.12, using the working-tree
Qualitygate 1.3.0 development binaries. This record covers a small, explicitly
bounded Rust fixture and the native acquisition path. It is not a measurement
of real-project detection quality, false-positive rate or operational latency.

## Pinned inputs

| Component | Actual input |
| --- | --- |
| crapper | [`c757ec79a6c15dc815238bbd9c2f4601eb54d3cc`](https://github.com/unclebob/crapper/tree/c757ec79a6c15dc815238bbd9c2f4601eb54d3cc) |
| dryer | [`66ff6d21a42c04afcad89c78a80066176d1294b0`](https://github.com/unclebob/dryer/tree/66ff6d21a42c04afcad89c78a80066176d1294b0) |
| tree-sitter-language-pack | 1.25.2, official Linux x86_64 CPython ABI3 wheel |
| Wheel SHA-256 | `d433d8b6dfa4fc3577eb3518b869af8dd81c22ceb625d7983fe08057902947fc` |
| Official parser bundle | [`parsers-linux-x86_64.tar.zst`, v1.25.2](https://github.com/xberg-io/tree-sitter-language-pack/releases/download/v1.25.2/parsers-linux-x86_64.tar.zst) |
| Bundle SHA-256 | `edcf1bc5546f263fadfd59e648ecb06023875eee164262bb41a805e0267a0b10` |
| Extracted Rust parser | `libtree_sitter_rust.so`, 1,478,320 bytes |
| Parser SHA-256 | `43b4551eedd58c54397f9571cacdefdac2ec077eaceb00a43977563c605d03f6` |
| Python tree-sitter package | 0.25.2 |

The upstream checkouts were unmodified. Installation and parser provisioning
were explicit preparation in temporary directories, outside the gate. The gate
neither installed dependencies nor fetched parser libraries. The interpreter,
configured launchers, tool version probes, selected snapshot and normalized
artifact identities were retained in the execution report.

An initial experiment with language-pack 0.9.0 failed because both upstream
tools import its later `download` API. The upstream loose dependency range
does not by itself establish compatibility; the accepted native profile pins
1.25.2 and the exact official parser bytes.

## Reproducible fixture and configuration

Create an isolated Git repository with Cargo package `probe`, version `0.1.0`,
edition `2024`, and this `src/lib.rs`:

```rust
pub fn alpha(x: i32) -> i32 {
    if x > 0 {
        x + 1
    } else {
        x - 1
    }
}
pub fn beta(x: i32) -> i32 {
    if x > 0 {
        x + 1
    } else {
        x - 1
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn checks_each_branch() {
        assert_eq!(alpha(1), 2);
        assert_eq!(alpha(-1), -2);
        assert_eq!(beta(1), 2);
        assert_eq!(beta(-1), -2);
    }
}
```

The exact source, including its final newline, has SHA-256
`8a29152e75d19da68a0928d9507be948cc60da1a195c910cc448f9f341f26d51`.
Track the preprovisioned parser as `vendor/rust.so` and declare it in
`verification_assets`. Ignore generated `target/`, `.metrics/`, and
`.qualitygate/` directories. Use `rulesets: []` to isolate command acquisition.

Configure three commands:

1. `coverage`: create `target/coverage/rust`, then run
   `cargo llvm-cov --lcov --output-path target/coverage/rust/lcov.info`.
   Its full LCOV report declares `coverage_paths: [src/lib.rs]`,
   `minimum_coverage: 100`, and `require_branch_coverage: false`. Probe
   `cargo llvm-cov --version` independently. This is a line-coverage fixture;
   it does not claim branch-coverage collection.
2. `crapper`: execute the pinned upstream CLI with
   `--use-existing-coverage src/lib.rs`; depend explicitly on `coverage`.
   Declare `function_metric`, `.metrics/crap.edn`, and
   `coverage_producer: coverage`.
3. `dryer`: execute the pinned upstream CLI with
   `--threshold 0.8 --min-lines 1 --min-nodes 0 src/lib.rs`.
   Declare `clone_pair`, `.metrics/dry.edn`, `minimum_lines: 1`,
   `minimum_nodes: 0`, and exact decimal floor `{coefficient: "8", scale: 1}`.

Both native declarations use `scope: {project: probe}`, `paths: [src/lib.rs]`,
`crate_name: probe`, the upstream commit as `tool_version`, and:

```yaml
native_runtime:
  parser_path: vendor/rust.so
  parser_digest: sha256:43b4551eedd58c54397f9571cacdefdac2ec077eaceb00a43977563c605d03f6
  pack_version: 1.25.2
```

The smoke launchers only import the original upstream `cli.main` using the
prepared interpreter. Their bytes and parser bytes are declared probe inputs.
Commit probes run `git -C <pinned-checkout> rev-parse HEAD`. A separate
`tree-sitter-language-pack` probe runs the same prepared Python interpreter
and prints `tree_sitter_language_pack.__version__`. Run:

```sh
qualitygate --root <isolated-fixture> check --worktree --format json --output-dir <external-evidence-directory>
```

## Observed outcomes

| Case | Gate exit | Observed result |
| --- | ---: | --- |
| Complete native acquisition | 0 | Three producers pass, zero incomplete. crapper publishes six complete metrics: each function has CC 2, coverage 100, CRAP 2. dryer publishes one complete pair, similarity 1, 47 nodes per endpoint, locations 1–7 and 8–14. |
| crapper declared findings exit | 0 | Add `--threshold 1` and `findings_exit_codes: [2]`. Actual crapper exit is 2; six complete metrics are retained and its producer passes. This verifies collection semantics, not a consumer rule verdict. |
| Missing parser snapshot input | 2 | Remove `vendor/rust.so`. Coverage passes; both native producers return `tool_error`, no verdict and no typed facts. Their tool-input preflight rejects the missing file before analyzer execution. |
| Wrong dependency version | 2 | Point the dependency probe at an extracted, importable 0.9.0 package. Coverage passes; both native producers return `tool_error` for the mismatched language-pack version, no verdict and no typed facts. |
| Offline bridge | 0 | Convert the actual raw EDN reports through `qualitygate-tool-bridge`. Six metrics and one clone pair survive; both envelopes retain `complete: false`. |

The successful acquisition bound both native fact envelopes to snapshot
`sha256:c71e3626ffc087d3c2f4ca214ae2e7ebeae38d0de72fc2070b6ce3857883c57d`.
The findings case has a different snapshot because its policy changed.

The native runtime creates a fresh private cache with only the pinned parser
and a deliberately invalid local manifest. It overrides `CACHE_DIR`,
`LIBS_DIR`, and `MANIFEST_URL` under the `TREE_SITTER_LANGUAGE_PACK_` prefix.
An additional direct upstream negative probe removed the parser from that
cache: the tool exited 1 with a local JSON parse error. The pinned upstream
[manifest transport](https://github.com/xberg-io/tree-sitter-language-pack/blob/v1.25.2/crates/ts-pack-core/src/download/transport.rs)
reads `file://` locally; this observation plus source review supports the
no-download profile. It is not a packet-level network-isolation experiment.

## Retained evidence and limits

Small logs, raw EDN, normalized envelopes, configuration, fixture source,
Python package inventory and `summary.json` are retained locally in
`target/tool-facts-native-verification/`. Large parser binaries are not copied
into this evidence directory or committed. Full original reports also retain
the verified parser artifact in their external run directories.

| Report | SHA-256 |
| --- | --- |
| Positive | `50445fbb604f1022ecaf9c92fe9178ff5a6d7ff52c4bf04ccdd658ad6e8c902b` |
| Findings exit | `24190c4ebaea674cdbea7e6f2373e8f2b8809458849a4cf32496cfdc2ad31ba9` |
| Missing parser | `763012f376482895bf56748d02fc2657c315ddaf6936826543e29540c3e6ad84` |
| Wrong package version | `0a27e871aec28942e3e89dd6886dba62219257cdf82c1dbdc14ed1224cc2434c` |

This adds live-tool evidence for TFA-A01, A08, A12 and A13. It does not replace
the independent native adapter/unit regressions, V4 consumer tests, full
coverage gate, Bazel, Miri or ASan records. No V4 assertion was configured in
this smoke repository. The fixture exercises simple free Rust functions and
excludes the test module as the audited tools do. Closures, generic impls,
other operating systems, other parser builds and general repository efficacy
remain outside this observation.

## Rounded acquisition boundary counterexample

Final source review identified a distinction between dryer's unrounded
acquisition filter and its serialized twelve-place score. An additional
unmodified-upstream experiment on these Rust functions confirmed it:

```rust
pub fn a(x: i32) -> i32 { if x > 0 { x + 1 } else { x - 1 } }
pub fn b(x: i32) -> i32 { if x > 0 { x.abs() } else { x - 1 } }
pub fn c(x: i32) -> i32 { x.abs() }
pub fn d(x: i32) -> i32 { let y = x + 1; y + 2 }
pub fn e(x: i32) -> i32 { if x > 0 { return x; } x + 1 }
```

With minimum lines 1 and nodes 0, the pair on lines 2 and 3 has internal score
`0.5263157894736842`. Acquisition at zero emits it as `0.526315789474`;
acquisition at `0.526315789474` omits it. Consequently equal-looking source and
acquisition thresholds cannot establish complete serialized-score selection.
The source, both raw reports and observation are retained in the local
`target/tool-facts-native-verification/rounding-boundary/` directory.
Using the same pinned interpreter, upstream checkout and private offline
environment described above, these CLI commands reproduce the captured EDN:

```sh
python -m dryer --threshold 0 --min-lines 1 --min-nodes 0 --edn lib.rs > unfiltered.edn
python -m dryer --threshold 0.526315789474 --min-lines 1 --min-nodes 0 --edn lib.rs > filtered.edn
```

Both commands exited zero. The raw score was additionally inspected through
the unmodified upstream `scan_files` and `find_duplicates` functions; the
CLI output matched their `render_edn` output byte for byte.

| Counterexample input/report | SHA-256 |
| --- | --- |
| `lib.rs`, exact five lines above with final newline | `fbdae59d41120d93d9ee16884ff80164f7d9f9d2c4391fffd412e16dc2ea518c` |
| `unfiltered.edn`, ten pairs | `d00734dc993acd276d4a26a22a7951c681a89d3d32c4d21162924401019dcbbd` |
| `filtered.edn`, one pair, omitting lines 2/3 | `8becc3e0cc72d4e358d3204b921a6a21342525a17635ba346d594cb9a591f5c2` |

`tool_evidence::contracts::clone_floor_covers` now checks the exact rounding
preimage of the smallest twelve-place value accepted by the source. For the
pinned dryer measurement, acquisition must be zero or no greater than
`ceil(source_floor * 10^12) / 10^12 - 0.5 * 10^-12`; normal scope containment
still applies. The check also covers generic envelopes declaring that same
measurement. It uses bounded integer arithmetic, not a floating-point epsilon.
The exact half-quantum boundary and values one `10^-18` step above/below have
pure regression tests; V4 validates compatibility at planning and runtime.

For a consumer floor of `0.82`, collection at `0.819999999999` retains the full
required rounding preimage. Collection at `0.82` remains a valid observation
of that upstream acquisition window, but cannot support a consumer asking for
the same serialized floor. The earlier successful producer-only smoke used
floor `0.8` without a V4 consumer, so its recorded result is unchanged.
