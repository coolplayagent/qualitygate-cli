# Qualitygate CLI

[简体中文](README.zh-CN.md) · [Documentation](docs/en/README.md)

Qualitygate CLI is a repository quality and task acceptance gate for developers,
CI, and coding Agents. It checks a selected Git snapshot against repository
policy, runs existing project tools, and returns evidence for the delivery decision.

## Why Qualitygate

- **Reproducible evidence:** results bind the snapshot, policy and task, so a pass
  cannot be reused for changed inputs.
- **Clear outcomes:** blocking violations and incomplete validation are distinct;
  missing reports, timeouts and unsupported capabilities remain visible.
- **One gate for repository rules and tools:** combine built-in rules,
  source-bound project rules derived from instructions such as `AGENTS.md`, and
  reports from existing linters, tests and coverage tools.
- **Traceable Agent acceptance:** bounded repair feedback leads back to a full
  check of the final snapshot, with separate policy review and task acceptance.

## Get started

Download the CLI or matching Agent Skill from the
[project site](https://coolplayagent.github.io/qualitygate-cli/). With Git and
`qualitygate` available on your PATH, initialize the repository:

```bash
qualitygate --root /path/to/repository init --format json
```

Review the candidate `qualitygate.yaml`, then check your changes:

```bash
qualitygate --root /path/to/repository check --worktree --profile quick --format json
qualitygate --root /path/to/repository check --worktree --profile full --format json
```

Use `quick` while editing and an unfiltered `full` check before delivery.
Exit `0` means complete pass, `1` a blocking violation, and `2` incomplete validation.
Continue with [installation and your first check](docs/en/01-user-guide/01-installation-and-first-check.md).
