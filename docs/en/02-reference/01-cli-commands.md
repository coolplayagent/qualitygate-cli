# CLI commands

[简体中文](../../zh/02-reference/01-cli-commands.md) · [Volume index](README.md)

Use `qualitygate --help` and subcommand help for the installed runtime's exact
options. Repository commands accept `--root`; examples below run from the root.
Use `--format json` for automation and inspect both exit status and report fields.
Uppercase identifiers and example paths must be replaced with your inputs.

## Initialization and inspection

```bash
qualitygate init --format json
qualitygate config --show --format json
qualitygate capabilities --format json
qualitygate doctor --worktree --profile full --format json
```

`init` creates a missing candidate configuration. `init --with-checks` also adds
command candidates. `config --show` and `capabilities` are read-only; default
`doctor` runs no project commands. `doctor --probe-tools` executes declared tool
probes. See [runtime and preflight](06-runtime-and-preflight.md) for diagnostics,
compatibility and initialization behavior.

## Rule discovery and authoring

```bash
qualitygate rules categories --format json
qualitygate rules list --language rust --source all --format json
qualitygate rules describe RULE_ID --format json
qualitygate rules schema --format json
qualitygate rules source --document AGENTS.md --section "Quality contract" --format json
qualitygate rules validate candidate.yaml --format json
qualitygate rules generate --input candidate.yaml --format json
```

`generate` writes a structured rule candidate; it does not turn arbitrary prose
into an adopted rule. `rules enable`, `rules disable` and `rules configure` mutate
candidate policy. Listing, describing, source binding, schema export and validation
are read-only. See [rules](02-rules.md) and the
[authoring tutorial](../01-user-guide/06-schema-guided-repository-rules.md).

## Snapshot checks

```bash
qualitygate check --staged --profile full --format json
qualitygate check --worktree --profile quick --feedback --format json
qualitygate check --diff BASE..HEAD --profile full --task task.yaml --format json
qualitygate check --mr URL --profile full --format markdown
```

Choose one snapshot selector; omitted selection uses the worktree. `--path`
narrows feedback. `--policy-ref` selects a resolved policy commit without proving
its approval. `--trust-store` and `--evidence-dir` supply caller-controlled signed
evidence. Check commands execute the project tools selected by policy.
[Exit codes and limits](05-exit-codes-and-resource-limits.md) define outcomes.

## Schemas and Agent integration

```bash
qualitygate schema capabilities --format json
qualitygate schema doctor --format json
qualitygate schema constraint --format json
qualitygate schema operator-rule --format json
qualitygate schema project-rule-family --format json
qualitygate schema agent-obligation --format json
qualitygate agent-contract --worktree --role implementer --format json
qualitygate agent-run --help
qualitygate check --worktree --profile full --export-delivery /external/delivery --format json
```

Query capabilities before using these interfaces. Schema export and contract
projection are read-only; projection does not run checks. `agent-run` launches a
configured Agent, may change its checkout and writes evidence. Delivery export
writes outside the repository. Follow [Agent contracts](09-agent-contracts.md)
and [host setup](10-agent-host-and-delivery.md) for required inputs and limits.

## Selfcheck and pilot evaluation

```bash
qualitygate selfcheck --format json
qualitygate pilot seal --input pilot-plan.json --format json
qualitygate pilot authorization-subject --input observations.json --format json
qualitygate pilot acceptance-subject --input observations.json --trust-store /external/trust.json --authorization /external/start.dsse.json --format json
qualitygate pilot summarize --input observations.json --format json
```

Selfcheck executes bundled fixtures against independent goldens. Pilot commands
validate supplied plans and records; they do not sign subjects or manufacture
observations. See [selfcheck](../04-contributor-guide/02-selfcheck-and-fixtures.md)
and [pilot evaluation](../04-contributor-guide/04-pilot-evaluation.md).

---

[Previous: Evaluating adoption in a real repository](../01-user-guide/05-real-repository-pilot.md) · [Contents](../README.md) · [Next: Built-in and custom rules](02-rules.md)
