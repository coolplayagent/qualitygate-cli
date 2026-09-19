# Qualitygate CLI

[简体中文](README.zh-CN.md)

Qualitygate CLI is a Rust command-line gate for repository policy and task
acceptance. It checks a selected Git snapshot, runs bounded project tools, and
returns reproducible evidence for developers, CI, and coding Agents.

## Why Qualitygate

- Binds every result to immutable Git snapshot, policy, and task identities.
- Distinguishes blocking violations from incomplete execution.
- Unifies built-in rules, schema-validated project rules, and external reports.
- Keeps policy, task, manual acceptance, and signed provenance traceable.
- Gives Agents bounded feedback without weakening the final full-check contract.
- Bounds time, concurrency, file acquisition, parsing, and process output.

## Quick start

Download the released CLI or matching Agent Skill from the
[project site](https://coolplayagent.github.io/qualitygate-cli/), then run:

```bash
qualitygate --root /path/to/repository init --format json
qualitygate --root /path/to/repository \
  check --worktree --profile quick --format json
```

Use an unfiltered `--profile full` check on the final snapshot before delivery.
Exit code `0` is a complete pass, `1` is a blocking violation, and `2` is
incomplete validation.

## Documentation

- [English book](docs/en/README.md)
- [中文文档](docs/zh/README.md)
- [Installation and first check](docs/en/01-user-guide/01-installation-and-first-check.md)
- [CLI reference](docs/en/02-reference/01-cli-commands.md)

Normative product requirements live in
[CodeSpec requirements](codespec/requirements/qualitygate-cli.md). Repository
navigation is governed by the [CodeSpec map](codespec/codespec-map.yaml) and
[Knowledge map](knowledge/knowledge-map.yaml).

## Development

```bash
cargo fmt --all -- --check
cargo check --all-targets --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo llvm-cov --all-targets --all-features --fail-under-lines 90
```

See the [contributor guide](docs/en/04-contributor-guide/README.md) for test,
architecture, documentation, selfcheck, Miri, and ASan boundaries.

## License

MIT
