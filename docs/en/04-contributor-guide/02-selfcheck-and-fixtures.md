# Selfcheck and fixtures

[简体中文](../../zh/04-contributor-guide/02-selfcheck-and-fixtures.md) · [Volume index](README.md)

`qualitygate selfcheck` is a bounded regression harness shipped with the CLI.
It materializes curated repositories, executes production rule/planning/report
paths, and compares normalized outcomes with independently authored golden
files.

```bash
qualitygate selfcheck
qualitygate selfcheck --fixture minimal --rule commit-message
```

The corpus covers passes, violations, missing evidence, malformed inputs,
timeouts, policy evolution, custom contracts, signed records, report adapters,
and compatibility behavior. Filters are useful for diagnosis but do not replace
a full corpus run after implementation changes.

Each result reports a verification conclusion, verified shapes, known limits,
and unverified assumptions. Agreement means no mismatch was found for those
fixture shapes. It is not proof that every repository, tool version, platform,
or external trust deployment works.

Fixtures must be deterministic, confined, and independent of user state. Git
fixtures initialize temporary repositories and local identity only. Network and
live-tool producers are separate explicitly invoked tests. Golden updates need
the same review as behavior changes; never regenerate expected output merely to
make a regression disappear.

When changing the CLI or its Skill package, run the matching published runtime's
full selfcheck, then the repository quality gates. Preserve mismatches and
execution gaps as separate findings. Installing a locally built development
runtime is a distinct, explicit operation and must not be described as a
published release.

The integration CLI helper removes inherited `CARGO_TARGET_DIR` from each child
process. Nested Cargo fixtures then use their own materialized snapshot target
directories, so concurrent fixtures with the same package name cannot exchange
test binaries or write into the parent build cache. This applies to the test
harness; the installed CLI continues to honor the project environment.

Packaged rule examples that bind a repository document must preserve its exact
source bytes on checkout. `.gitattributes` pins the V3 operator reference to LF;
the executable example test also forces `core.autocrlf=true` in its temporary
repository and verifies that checkout preserves the bound source hash.

---

[Previous: Development and quality gates](01-development-and-quality-gates.md) · [Contents](../README.md) · [Next: Requirement-to-test evidence](03-requirements-to-test-evidence.md)
