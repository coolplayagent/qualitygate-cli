# Installation and first check

[简体中文](../../zh/01-user-guide/01-installation-and-first-check.md) · [Volume index](README.md)

Qualitygate CLI checks a selected Git snapshot against repository policy and,
when supplied, a task contract. Git is required. Configured project commands
also require their own toolchains.

## Install

Download a release binary or the version-matched Agent Skill archive from the
[project site](https://coolplayagent.github.io/qualitygate-cli/). Contributors
can build the checked-out source:

```bash
cargo build --locked
cargo run -- --version
```

The released Skill contains its own matching executable and rule assets. Do not
silently substitute a binary built from an unrelated checkout.

## Discover a candidate policy

Run discovery in the repository you want to check:

```bash
qualitygate --root /path/to/repository init --format json
qualitygate --root /path/to/repository init --with-checks --format json
```

`init` inventories languages, build manifests, existing commands, analyzer
reports, and capability gaps. It may create a candidate `qualitygate.yaml`; it
does not approve that candidate, infer exemptions, or declare a team policy.
Review suggested commands and unsupported project shapes before adoption.

Before initialization, `config --show` reports incomplete validation. Table and
Markdown output include `Run: qualitygate init`; JSON retains `gate.blockers`.
`init` also emits an advisory `snapshot_preflight` over HEAD and eligible
worktree metadata, including tracked files ignored by discovery. It lists up to
100 oversized/unsupported entries of each kind, counts all observed entries,
and marks truncated details or incomplete inspection explicitly. This does not
validate arbitrary diff/MR endpoints, staged bytes, total budgets or toolchains.

For existing files above the default 2 MiB limit, follow the suggested
`--snapshot-max-file-mib N` (1–8 MiB) on `check`. This is caller-controlled
acquisition capacity, not a policy exemption. `--with-checks` adds reviewed
command candidates; it neither installs their dependencies nor runs them.
Review the candidate once within the authorized adoption scope; ordinary
acquisition retries do not require editing repository rules.

Discovery is bounded. Unreadable files, malformed manifests, unsupported
ecosystems, and exhausted budgets remain explicit gaps rather than successful
fallbacks.

## Run the first check

```bash
qualitygate --root /path/to/repository \
  check --worktree --profile quick --format json
```

This captures tracked changes and in-scope untracked files, materializes an
immutable execution input, and returns structured findings. Quick feedback can
guide a repair, but it cannot establish delivery readiness. Before delivery,
run an unfiltered `--profile full` check against the intended final snapshot.

The result separates three outcomes: a complete pass, a complete check with a
blocking violation, and incomplete validation. Read the report's snapshot,
policy, scope, pending checks, warnings, and evidence references together.
