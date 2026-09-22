# Qualitygate CLI Skill

This ClawHub-compatible skill teaches agents to use the published
`qualitygate` executable for snapshot-bound repository policy and task
acceptance checks and for Schema-guided translation of repository instructions
such as `AGENTS.md` into structured project-rule candidates. It is a decision
and safety layer over the CLI, not a replacement for its executable validation,
repository policy, protected references, human approval, or evidence.

## Distribution shape

The tracked skill directory is intentionally lightweight:

- [SKILL.md](SKILL.md) contains executable-agent guidance and version metadata.
- [agents/openai.yaml](agents/openai.yaml) supplies optional UI metadata.
- [operations](references/operations.md) records task, snapshot, trust, and
  result-handling boundaries.
- [bundled rule guide](references/builtin-rules.md) tells agents how to select
  the shipped general-language, quality, security, and architecture rules.
- `references/rules/**` contains the versioned built-in YAML assets read by the
  matching CLI release at runtime.
- [rule authoring](references/rule-authoring.md) and the executable
  [project rule schema](references/schemas/project-rule.schema.json) guide
  source-bound, schema-validated extraction from repository prose into
  `qualitygate/rules`; the matching CLI remains authoritative for Schema
  export, source binding, validation, and generation.
- [decision protocol](references/decision-protocol.md) and the pinned
  [decision](references/schemas/decision.schema.json) and
  [feedback](references/schemas/feedback.schema.json) schemas describe
  opt-in machine envelopes and external warning assessment.
- [file contracts](references/file-contracts.md) cover instruction budgets,
  unchanged file inventories, required files and evidence-driven repair.
- [diagnostic ratchets](references/diagnostic-ratchets.md) cover comparable
  analyzer runs, count growth, report evidence and rechecks.
- [rule management](references/rule-management.md) covers policy context,
  candidate changes, lifecycle and signed adoption; [selfcheck](references/selfcheck.md)
  covers bundled runtime regression.
- [pilot evidence](references/pilot-evidence.md) and the bundled
  [v10 manifest template](assets/pilot/observation-v10.json) cover trial sealing,
  attempt model and execution records, and independently signed acceptance.
  The [v7 template](assets/pilot/observation-v7.json) and
  [v8 template](assets/pilot/observation-v8.json) and
  [v9 template](assets/pilot/observation-v9.json) remain for older priced plans.

A tag release creates `qualitygate-cli-skill-<tag>.tar.gz`. Its root is this
skill directory plus these platform-specific runtime assets:

```text
qualitygate-cli-skill-<tag>/
├── SKILL.md
├── README.md
├── agents/openai.yaml
├── references/operations.md
├── references/builtin-rules.md
├── references/rule-authoring.md
├── references/file-contracts.md
├── references/diagnostic-ratchets.md
├── references/ruff-ratchet.yaml
├── references/gcc-analyzer-ratchet.yaml
├── references/clang-static-analyzer-ratchet.yaml
├── references/{checkstyle,pmd,spotbugs}-ratchet.yaml
├── references/rule-management.md
├── references/selfcheck.md
├── references/pilot-evidence.md
├── references/decision-protocol.md
├── references/schemas/project-rule.schema.json
├── references/schemas/decision.schema.json
├── references/schemas/feedback.schema.json
├── references/rules/{core,shared,lang-java,lang-python,lang-typescript,lang-go,lang-c,lang-cpp}/*.yaml
├── assets/pilot/observation-v7.json
├── assets/pilot/observation-v8.json
├── assets/pilot/observation-v9.json
├── assets/pilot/observation-v10.json
├── assets/linux-x86_64/qualitygate
├── assets/linux-aarch64/qualitygate
├── assets/windows-x86_64/qualitygate.exe
└── assets/windows-aarch64/qualitygate.exe
```

Agents prefer the matching bundled asset only when it is executable and
`qualitygate --version` succeeds. The Windows asset is for PowerShell or cmd,
not POSIX shells. A registry distribution may omit the binaries to respect
registry file limits; it then uses a verified published `qualitygate` executable
on `PATH` rather than building an arbitrary checkout. In both distributions,
the rule assets and selection guide stay with the skill. Complete archives resolve
their own rule assets. PATH fallback or custom layouts can set
`QUALITYGATE_BUILTIN_RULES_DIR`; agents compare the loaded catalog with `rules list`
before recommending a policy change. The Skill provides PowerShell and Linux Bash
startup examples that select the matching bundled architecture.

## Release contract

The repository's tag-triggered release workflow requires a semantic-version
tag that matches the CLI package version. It runs Rust quality
gates and package verification, builds Linux x64/ARM64 and Windows x64/ARM64
binaries, smoke-tests the natively runnable Linux variants and Windows x64, and
verifies the Windows ARM64 PE architecture before injecting all four assets into
the release skill archive. It verifies archive contents before publishing the
GitHub Release. If `CLAWHUB_TOKEN` is configured, the same versioned lightweight
skill is published to ClawHub after the GitHub Release succeeds.

`workflow_dispatch` creates a dry-run artifact only. It does not publish a
GitHub Release or contact ClawHub. A real public release still requires a
maintainer-controlled tag and credentials.

## Operating boundary

Use [SKILL.md](SKILL.md) for normal agent operation. In particular, do not use
the skill to write a policy, choose a trusted reference, manufacture manual
evidence, downgrade a failure, or publish external results unless the user has
separately authorized that action.

Existing repositories: follow [the trimming workflow](references/operations.md#existing-repository-trimming).
Checks derive delivery from the chosen snapshot; use reviewed top-level `exclude`
globs to omit historical resources from acquisition and tool inputs. Preserve the
report selection and exclusion evidence. The profile selects checks, not scope.
