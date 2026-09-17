# Skill-over-CLI release package

Qualitygate is published as a CLI-operated skill: the agent receives a narrow,
versioned workflow and invokes the published `qualitygate` executable rather
than reimplementing gate semantics in prompts. The tracked package is
[skills/qualitygate-cli](../skills/qualitygate-cli/README.md).

## Package boundary

The source package contains `SKILL.md`, OpenAI-compatible UI metadata, focused
operations and built-in-rule references, plus every built-in rule YAML under
`references/rules/**`. It also bundles the pilot evidence reference and v7
manifest template, so a published Skill does not depend on repository-only
implementation documents or templates. The source package also includes the v8,
v9 and v10 pilot manifest templates. The CLI reads rule assets at runtime through the
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

Version v0.5.0 is the first release after the v10 unpriced pilot contract.
The [project site](https://coolplayagent.github.io/qualitygate-cli/) is deployed
from [`site/`](../site/index.html) on `main` by the separate
[Pages workflow](../.github/workflows/pages.yml). It links to the matching
release archive, install instructions, and repository documentation. Publishing
the site does not publish a Skill release; only the validated version-tag
workflow does that.

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

Version 0.3.0 adds the selfcheck workflow reference and compiles the repository
fixture/golden corpus into each platform binary. The installed binary can run
selfcheck without a source checkout. A user-requested development update may
replace the local Skill runtime after full verification, with the previous
runtime preserved; this does not publish a release.

`tests/quality/skill_package.rs` validates the package structure, version
alignment, UI metadata, safety instructions, bundled Markdown links and release-workflow asset
contract. It runs with the repository documentation/architecture quality gate;
the release workflow validates the produced archive again before it is
published.

The Skill entrypoint and UI prompt expose file contracts and diagnostic
ratchets, with self-contained references for configuration, execution, repair
and limits. The release archive checks retain those references alongside rule
management and selfcheck. Development builds can share a version string, so
agents compare the exported project schema with the shipped schema and validate
report configuration using the selected runtime before execution.

| Skill requirement | Verification |
|---|---|
| Instruction/file contract example is accepted only with its full-inventory mode | `tests/quality/skill_package.rs::skill_capability_examples_match_runtime_validation` parses the shipped YAML, supplies a fixture digest, and rejects changing `all` to `any` |
| Ratchet example requires fresh baseline configuration | The same test validates the shipped command configuration and rejects removing `baseline` |
| Package references and schema reach the agent | Package structure/version test, documentation link gate and release archive file checks |
| Published Skill resolves its own documents and pilot templates | `published_skill_has_only_bundled_document_references_and_template` checks every Markdown link stays inside the package, rejects repository-only pilot paths, and compares bundled v7/v8/v9/v10 templates byte-for-byte with repository templates; release packaging checks all files |
| Actual gate outcomes and incomplete evidence | Separate `file_contracts` and `ratchet` integration suites; full selfcheck remains evidence for its bundled corpus only |
| DIST-01: versioned GitHub Action release | `skill_package_contract_is_complete_and_matches_the_cli_version`, Bazel version test, tag workflow, and published release archive/checksum inspection |
| DIST-02: Pages installation and documentation entry | `tests/quality/site.rs` checks version, local assets, repository links, anchors, and workflow source/permissions; Pages run and live URL are checked after deployment |

The package validator accepts both LF and CRLF YAML frontmatter delimiters,
including a closing delimiter at end of file. It preserves the original YAML
bytes and still rejects missing, indented or malformed delimiters; Windows
checkout line endings do not exempt any metadata or safety assertion.
