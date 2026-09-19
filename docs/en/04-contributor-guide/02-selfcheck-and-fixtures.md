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
