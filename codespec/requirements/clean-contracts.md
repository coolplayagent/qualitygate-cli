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
