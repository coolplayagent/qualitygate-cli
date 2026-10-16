# Installation and first check

[简体中文](../../zh/01-user-guide/01-installation-and-first-check.md) · [Volume index](README.md)

This chapter takes an existing Git repository from installation to its first
full report. You need Git and any toolchains required by the repository's checks.
Commands below use `/path/to/repository` for the repository you want to verify.

## 1. Install and identify the runtime

Download the CLI archive for your operating system and architecture from the
[project site](https://coolplayagent.github.io/qualitygate-cli/). Extract the
complete archive so the executable and rule assets stay together, and add its
executable directory to PATH. For an Agent integration, use the matching Skill
archive and its bundled executable.

```bash
qualitygate --version
qualitygate capabilities --format json
```

Capability IDs identify available interfaces; a version string alone does not
identify a development build. Source builds are covered in the
[contributor guide](../04-contributor-guide/01-development-and-quality-gates.md).

## 2. Initialize and review the candidate

```bash
qualitygate --root /path/to/repository init --format json
qualitygate --root /path/to/repository config --show --format json
```

`init` discovers languages, build manifests, available checks and capability gaps,
and creates a missing candidate `qualitygate.yaml`. Review the selected rules and
profiles. To include discovered command candidates, run `init --with-checks`
within your team's policy adoption process; it does not install or run those tools.
An existing valid policy can be inspected directly without another initialization.

Candidate creation is not policy approval. In particular, a pass only covers
configured checks: initialization alone does not guarantee that your tests or
coverage tools are selected. Continue with [rule configuration](03-rule-configuration.md)
when adapting the candidate to your repository.

## 3. Check prerequisites, then get feedback

```bash
qualitygate --root /path/to/repository doctor --worktree --profile full --format json
qualitygate --root /path/to/repository check --worktree --profile quick --format json
```

Default `doctor` inspects prerequisites without running project commands.
Resolve reported gaps before checking. `quick` runs the profile's selected checks
against captured worktree bytes, including eligible untracked files. It is useful
while editing; pending delivery checks remain visible in the report.

## 4. Verify the final snapshot

```bash
qualitygate --root /path/to/repository check --worktree --profile full --format json
```

| Exit | What happened | Next action |
| --- | --- | --- |
| `0` | The selected scope passed completely | Retain the report, warnings and evidence limits. |
| `1` | A blocking violation was found | Repair the finding and check again. |
| `2` | Validation could not finish | Restore prerequisites or evidence and retry. |

For delivery, use an unfiltered full report with no pending delivery checks.
A later edit requires a new check. The next chapter explains
[snapshots, report identities and delivery scope](02-snapshots-and-check-workflow.md).

If acquisition stops on large or unsupported files, consult
[runtime and preflight](../02-reference/06-runtime-and-preflight.md) before changing
budgets or exclusions. Keep the report's incomplete reasons; do not interpret
an interrupted check as a clean result.

---

[Contents](../README.md) · [Next: Snapshots and the check workflow](02-snapshots-and-check-workflow.md)
