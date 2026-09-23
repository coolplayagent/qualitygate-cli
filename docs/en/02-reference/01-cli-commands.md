# CLI commands

[简体中文](../../zh/02-reference/01-cli-commands.md) · [Volume index](README.md)

All repository commands accept `--root`. Use `--format json` for automation and
Markdown for human-readable reports.

## Repository setup and inspection

```bash
qualitygate --root . init [--with-checks] --format json
qualitygate --root . config --show --format json
qualitygate --root . rules categories --format json
qualitygate --root . rules list [--language rust] [--source all] --format json
qualitygate --root . rules describe RULE_ID --format json
qualitygate --root . rules schema
qualitygate schema command-error
qualitygate --root . rules validate candidate.yaml
qualitygate --root . rules generate --input reviewed-source.md
```

`init`, `rules enable`, `rules disable`, and `rules configure` mutate candidate
repository configuration. Listing, describing, schema export, and effective
configuration inspection are read-only.

## Checks

See [command prerequisites and recovery](06-command-prerequisites.md) for
initialization requirements and structured early errors.

```bash
qualitygate --root . check --staged --profile full --format json
qualitygate --root . check --worktree --profile quick --feedback --format json
qualitygate --root . check --diff BASE..HEAD --profile full --task task.yaml
qualitygate --root . check --mr URL --profile full --format markdown
```

Exactly one snapshot selector is used. `--path` narrows feedback. `--policy-ref`
selects policy from a resolved commit but is not proof that the commit is
approved. `--trust-store` and `--evidence-dir` point at caller-controlled inputs
for signed evidence and should remain outside the checked repository.

## Selfcheck and pilot evidence

```bash
qualitygate selfcheck [--fixture NAME] [--rule RULE_ID]
qualitygate pilot seal --input pilot-plan.json --format json
qualitygate pilot authorization-subject --input observations.json --format json
qualitygate pilot acceptance-subject --input observations.json \
  --trust-store /external/trust.json \
  --authorization /external/start.dsse.json --format json
qualitygate pilot summarize --input observations.json --format json
```

Selfcheck compares bundled fixtures with independently authored goldens. Pilot
commands validate and summarize supplied records; they do not create trust
keys, sign subjects, approve policy, or fill in missing real-world evidence.

Use `qualitygate --help` and subcommand help for the version-specific option
surface. Scripts should check both the process exit code and the structured
gate fields.
