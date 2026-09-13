# Skill-over-CLI release package

Qualitygate is published as a CLI-operated skill: the agent receives a narrow,
versioned workflow and invokes the published `qualitygate` executable rather
than reimplementing gate semantics in prompts. The tracked package is
[skills/qualitygate-cli](../skills/qualitygate-cli/README.md).

## Package boundary

The source package contains `SKILL.md`, OpenAI-compatible UI metadata, focused
operations and built-in-rule references, plus every built-in rule YAML under
`references/rules/**`. The CLI reads those assets at runtime through the
Skill-owned `QUALITYGATE_BUILTIN_RULES_DIR` path (or the adjacent release
layout); it contains no compiled rule manifest fallback. The assets make the
general-language, quality, security, and architecture definitions reviewable
by an agent even when a registry omits runtime binaries. `rules list` remains
the executable authority. The source package is safe for lightweight registries
because it contains no checked-in executable assets. A release bundle adds the
platform binaries needed by the skill:

| Runtime | Release archive path | Shell boundary |
| --- | --- | --- |
| Linux x64 | `assets/linux-x86_64/qualitygate` | POSIX shells |
| Linux ARM64 | `assets/linux-aarch64/qualitygate` | POSIX shells |
| Windows x64 | `assets/windows-x86_64/qualitygate.exe` | PowerShell or cmd |
| Windows ARM64 | `assets/windows-aarch64/qualitygate.exe` | PowerShell or cmd |

At runtime, the skill probes the matching bundled asset with `--version` and
falls back to a verified published `qualitygate` on `PATH` only when the asset
is absent or unusable. It never treats a source checkout as an installation and
does not execute the Windows asset through a POSIX shell.

## Release workflow

[`.github/workflows/release.yml`](../.github/workflows/release.yml) runs on a
version tag matching the Cargo package version or as a dry-run dispatch. It:

1. verifies the version and Rust format, check, Clippy, test, coverage, and
   package gates;
2. builds Linux x64/ARM64 and Windows x64/ARM64 release binaries, smoke-tests
   native Linux variants and Windows x64, and verifies the Windows ARM64 PE
   architecture;
3. creates and validates `qualitygate-cli-skill-<tag>.tar.gz`, including the
   exact rule mirror, selection guide, expected instructions, and matching
   binaries;
4. creates a GitHub Release only for a tag push; and
5. publishes the asset-free source skill to ClawHub only when a maintainer has
   configured `CLAWHUB_TOKEN`.

The workflow does not use its dry-run dispatch to publish a release. It also
does not create a package version or tag: versioning and release authority
remain with maintainers.

## Agent safety contract

The [skill](../skills/qualitygate-cli/SKILL.md) keeps policy changes and actual
checks distinct. Listing rules and configuration is read-only; `init` and
`rules enable` require an explicit mutation request. It requires an agent to
read the bundled rule guide, verify the compiled catalog, and avoid inventing
severity or architecture boundaries. A check reports violations and incomplete
execution separately, preserves snapshot and policy evidence, and never turns
missing tools, stale reports, or manual-approval gaps into a pass. Task,
trusted-policy, merge-request, and manual-evidence boundaries are specified in
its [operations reference](../skills/qualitygate-cli/references/operations.md).

## Verification

`tests/quality/skill_package.rs` validates the package structure, version
alignment, UI metadata, safety instructions, and release-workflow asset
contract. It runs with the repository documentation/architecture quality gate;
the release workflow validates the produced archive again before it is
published.
