# Rust practice lane

## Source coverage

The Rust API Guidelines are a review checklist for idiomatic, interoperable
public APIs. They explicitly describe themselves as guidance rather than a
universal mandate.[1] Cloudflare reports that its governed standards have
language domains including Rust and separates approved advice from enforced
requirements.[2]

## Current rule inputs

Qualitygate can parse Rust test attributes for generic test naming,
parameterization, comments, and provenance checks. It has no packaged Rust API
or compatibility rule. The matrix therefore records rust-public-api-review as
planned rather than claiming that a generic source scan can judge Rust API
design.

## Promotion requirements

Before enforcing a Rust public-API rule, define:

- the crate's public API surface and compatibility policy;
- a rustdoc or API-diff artifact bound to the selected snapshot;
- exceptions for intentionally unstable or internal APIs;
- diagnostics that distinguish a policy violation from absent API evidence.

Rust's ownership and type system reduce some bug classes but do not replace
authorization, threat modeling, dependency review, benchmark evidence, or
operational objectives.

## Sources

1. Rust Project, [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/).
2. Cloudflare, [How Cloudflare enforces engineering standards using AI](https://blog.cloudflare.com/engineering-standards-enforcement/).

[1]: https://rust-lang.github.io/api-guidelines/
[2]: https://blog.cloudflare.com/engineering-standards-enforcement/
