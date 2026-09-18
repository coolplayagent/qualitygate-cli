# Rust Project

Source: [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)

Authority: A Rust library-team-authored public API review guide. It explicitly
states that its recommendations are advisory and may vary with crate context.

## Rule inputs

- Public API shape, naming, documentation, interoperability, and future
  compatibility are review inputs for Rust libraries.
- A compatible quality gate needs an adopted public-API policy and a
  snapshot-bound rustdoc or API-diff artifact.
- Rust ownership and type checks are useful language evidence but cannot prove
  architecture, authorization, benchmark results, or operational objectives.

Qualitygate records Rust public API review as planned. It will not turn this
guide into a blocking rule until a repository explicitly defines compatibility
scope, exceptions, evidence, and severity.

The [Rust Reference on external blocks](https://doc.rust-lang.org/reference/items/external-blocks.html)
specifies that an omitted ABI defaults to `"C"` and recommends explicit ABI
syntax. Its [macro syntax](https://doc.rust-lang.org/reference/macros-by-example.html)
and [unsafe keyword](https://doc.rust-lang.org/reference/unsafe-keyword.html)
define the constructs inspected by the optional Rust source patterns. The
[libloading Library API](https://docs.rs/libloading/latest/libloading/struct.Library.html)
documents that loading a dynamic library executes its initialization routines.
These sources justify review signals; the single-line patterns do not prove
that a path is untrusted, a macro is unsafe at a call site, or FFI behavior is
sound. For semantic lint findings, the Clippy ratchet uses a fresh analyzer
run on each snapshot and retains its raw Cargo JSON Lines evidence.
