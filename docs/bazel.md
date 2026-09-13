# Bazel build and dependency checks

Cargo remains the supported default build and package manager. `Cargo.toml`,
`Cargo.lock`, and `rust-toolchain.toml` are the single sources of truth for
the package, resolved Rust crates, and compiler version. Bazel is an additional
entry point for reproducible target builds and cache reuse; it does not replace
Cargo or introduce a second hand-maintained dependency list.

## Local commands

```bash
# Existing Cargo workflow remains unchanged.
cargo build --locked
cargo test --locked --all-targets --all-features

# Resolve Bzlmod dependencies without allowing lockfile changes.
bazel mod deps --lockfile_mode=error

# Build the CLI and run the Bazel-specific Rust tests.
bazel build --lockfile_mode=error //:qualitygate
bazel test --lockfile_mode=error //:unit_tests //:bazel_contract_test
```

The first local Bazel bootstrap may need to create `MODULE.bazel.lock`. Run the
following once after changing `MODULE.bazel`, then review and commit the lock:

```bash
bazel mod deps --lockfile_mode=update
```

When Cargo dependencies change, refresh Cargo's lock in the normal way, then
refresh Bazel's generated crate repository and module lock before running the
strict commands. Bazel 9 performs this during target analysis; its legacy
`sync` subcommand is not available:

```bash
CARGO_BAZEL_REPIN=1 bazel build --lockfile_mode=update //:qualitygate
bazel mod deps --lockfile_mode=update
```

`crate.from_cargo` consumes the checked-in Cargo manifest and lock directly.
Consequently, a stale Cargo lock, an unresolved Bazel module, or a source target
whose declared crate dependencies no longer match the Cargo graph fails Bazel
analysis or the strict module-lock check. `tests/bazel.rs` additionally guards
the configuration contract under the normal Cargo test suite.

## Cache scope

The repository `.bazelrc` enables two user-local caches:

- `repository_cache` stores downloaded Bzlmod modules and crate archives.
- `disk_cache` stores completed action outputs, including the CLI library,
  binary, and test actions.

Their paths are below `~/.cache/qualitygate-cli/bazel`, so they are outside the
working tree and can be overridden with Bazel command-line flags. The CI job
also restores Bazelisk, external-module, repository, and disk caches.

Cargo still packages one CLI crate, while Bazel compiles the source ownership
graph as separate libraries. Each top-level owner has a named directory and a
BUILD.bazel file below src/qualitygate: domain, env, paths, net, runner,
snapshot, config, adapters, application, and interfaces. The top-level
qualitygate target is only a facade that re-exports those libraries for the
existing binary path.

Nested source areas stay under their owning domain and declare their own source
groups, including adapters/reports, adapters/syntax, and config/discovery. A
change in a domain library invalidates that library and its downstream
consumers, instead of recompiling unrelated leaf domains. The shared Bazel
dependency adapter deliberately reads the one checked-in Cargo dependency
graph, so these source packages do not introduce package-local Cargo manifests
or a second dependency lock.

.bazelignore excludes Cargo's generated target tree (including Cargo
package-verification copies that contain BUILD.bazel) from Bazel package
discovery. This keeps bazel test //... scoped to the repository's declared
targets after either build system has run.

## CI evidence

The Bazel workflow runs, in order, strict module dependency resolution, the
CLI build, unit tests, and the Bazel/Cargo contract test. Cargo quality gates
remain independently defined in the existing workflow. The full profile in
qualitygate.yaml also runs locked Bazel module, build, and test checks after
the existing Cargo checks.
