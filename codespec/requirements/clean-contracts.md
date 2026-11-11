# Clean contract maintenance requirements

This supplement applies the repository ownership and quality requirements to
the code/rule audit. The [research notes](../../knowledge/best-practices/engineering-standards/guides/clean-contracts.md)
record sources and distinguish design judgement from executable evidence.

| ID | Requirement | Acceptance evidence |
| --- | --- | --- |
| CLEAN-01 | Pure domain types must not depend on Clap. CLI accepted values, defaults, help and serialized domain values remain stable. | Typed interface parser tests and the production-source architecture gate. |
| CLEAN-02 | Rule discovery and validation share language and pattern constraints. Fixed scopes, explicitly permitted subsets, regex uniqueness and byte budgets remain enforced. Direct adapter calls must reject unsupported syntax languages. | Domain parameter tests, config discovery/validation tests, adapter failure tests and integration rule fixtures. |
| CLEAN-03 | Rule-specific source-pattern wording belongs to bounded, closed builtin metadata. It must not override evidence, verdict, severity or scope; policy parameters cannot replace it. Legacy catalog replay retains applicable compatibility behavior without rewriting stored definitions. | Catalog rejection and replay tests, literal guidance tests and the existing issue64 diagnostics/goldens. |
| CLEAN-04 | Canonical SHA-256 identities retain exact input bytes, prefix and lowercase hexadecimal representation. Streaming hash framing and public wrapper contracts stay stable. | Known vectors, malformed/non-ASCII values, exact network-body identity and attestation tests. |
| CLEAN-05 | Package discovery and selfcheck use one supported-package inventory. Typed fixture registration preserves path/byte order and independent expected outcomes. | Selfcheck inventory tests, full selfcheck and same-checkout corpus digest comparison. |
| CLEAN-06 | CLI root filesystem resolution delegates through the application blocking boundary and `paths`, preserving native canonicalization and missing-root context. Architecture checks reject reserved path-I/O method forms in domain/interfaces and emit file/line evidence. | Path/application root tests and architecture regression fixtures, including aliased static calls and macro method calls. |
| CLEAN-07 | Isolated fixtures establish native path and Git-byte preconditions explicitly. Environment budget tests exercise the real digest algorithm without depending on the OS accepting an oversized process environment. Windows write denial and Unix content-tamper detection remain independently asserted. | Canonical temporary-root fixtures, exact index/commit byte assertions, protected-path rejection, command/probe guard tests and `env::tests`. |

The empty subset-language loophole is intentionally rejected: an empty selection
must not mean unrestricted execution with an `all` pattern. Renaming a declared
source-pattern definition must not remove its language constraint. Unsupported
C/C++ import patterns are rejected by direct adapter entry points as well as
configuration validation; this does not add a C/C++ import parser.

The architecture check is syntactic. Reserved path method names are conservative
boundary constraints, not receiver type inference or proof that all possible
external I/O has been detected. Context-sensitive clean-code judgements remain
review obligations. No rule or coverage threshold may be weakened to make this
refactor pass. Stable unit/integration checks, coverage, Bazel and a full final
snapshot check remain required; nightly Miri and ASan have separate CI evidence.

Business-module refactoring keeps the same public contracts and capability
boundaries. Implementations live directly in explicitly named first-level
business modules, without nested role wrappers. Single-module owners use flat
`.rs` files. The five historical role APIs
are flat compatibility files. Each concrete module has a reviewed capability
owner; pure modules stay free of I/O and interface modules delegate I/O.
Unregistered modules and cross-owner cycles remain errors. The architecture
regression `architecture_business_modules_preserve_capability_and_dependency_boundaries`
checks rejection behavior; the full production graph retains source digests and
file/line evidence. Existing integration tests exercise the historical public
paths while Bazel builds and tests the independently compiled owner crates.

Adapter implementations follow the same first-level business ownership. Report
formats, source facts, Maven/Python project analysis, rule evaluation and external
acceptance have distinct owners. Module and file names use descriptive snake_case;
modules with children use directory/mod.rs. Remove shallow role wrappers and
empty migration directories; preserve public adapter contract identity through
flat facades in Cargo and Bazel. The adapter-boundary and source-layout
architecture tests plus `business_adapter_paths_preserve_the_public_report_contract`
provide the corresponding rejection, layout and compatibility evidence.
The Miri workflow inventory must equal the reviewed pure-module inventory, and
each shard must reject an empty test selection before executing it. Renaming a
module must not silently remove its tests from the pure-domain gate.
