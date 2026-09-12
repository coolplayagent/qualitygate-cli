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
