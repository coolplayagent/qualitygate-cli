# Qualitygate CLI Skill

This ClawHub-compatible skill teaches agents to use the published
`qualitygate` executable for snapshot-bound repository policy and task
acceptance checks. It is an instruction layer over the CLI, not a replacement
for repository policy, protected references, human approval, or evidence.

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
  schema-validated extraction into `qualitygate/rules`.
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
├── references/rule-management.md
├── references/selfcheck.md
├── references/pilot-evidence.md
├── references/schemas/project-rule.schema.json
├── references/rules/{core,shared,lang-java,lang-python,lang-typescript,lang-go,lang-c}/*.yaml
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
the rule assets and selection guide stay with the skill. The Skill sets
`QUALITYGATE_BUILTIN_RULES_DIR` before execution, and agents compare the loaded
catalog with `rules list` before recommending a policy change.

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
