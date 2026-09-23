# Command prerequisites and recovery

[简体中文](../../zh/02-reference/06-command-prerequisites.md) · [Volume index](README.md)

Commands check their actual dependencies. A valid manually authored configuration
counts as initialized; no record of a previous `init` invocation is required.
Qualitygate does not initialize, install tools, stage files, or alter trust inputs
as an automatic recovery action.

| Commands | Required inputs |
| --- | --- |
| Help, version, schema exports, `policy evaluator` | Their own bundled data or executable identity; no repository configuration. |
| `selfcheck` | Its fixture tools, assets and temporary storage; no target policy. |
| `init` | An accessible directory, discovery assets and candidate write access. Git preflight remains advisory. |
| `rules list/describe/categories/context` | Rule assets; default configuration is optional for discovery. An explicit custom configuration or policy ref must exist. |
| `rules source/validate/generate` | Selected files, schema and source bindings; publication needs write access. Existing active-policy authentication still applies. |
| `config --show`, local rule/category edits | Effective policy for display, local candidate for edits. Edits check configuration before acquiring their write lock and reread it under the lock. |
| `check` | Git, selected policy/snapshot, task and plan, then each selected check's actual dependencies. |
| Archive/candidate/lifecycle operations | Their archive objects, state and authorization. Evidence retention can create an archive. |
| `pilot`, `judgment` | Their selected inputs, output storage and applicable external evidence/provider. |

Missing local configuration returns `repository_not_initialized` with an exact
`init` argv preserving `--root` and `--config`. A local file missing from the
index returns `policy_snapshot_missing` and guidance to review/stage it. A
historical policy missing configuration requires a suitable ref; initializing
the current directory cannot modify a historical snapshot. A valid active
policy remains authoritative without a local YAML file. Authentication failures
never fall back to a local candidate.

Tool and cwd checks run after dependencies, because an earlier check can create
them. Only selected checks probe their tools. Tool failure preserves required
versus optional gate semantics. Reports are checked after production, including
freshness, integrity and snapshot identity; preflight cannot prove execution.

## Machine contract

Export `qualitygate schema command-error` for `urn:qualitygate:command-error:1`.
Early operational failures have `kind: command_error`, `schema_version: 1`, the
existing incomplete `gate` and `verification`, and typed `issues`. Each issue has
`code`, `phase`, `message`, optional `resource`, `check_id`, `cause`, and
`next_actions`. Actions are either `instruction` with a message or `command`
with a message and a complete `argv` array. Treat argv elements as arguments,
not interpolated shell code. Exit status remains 2.

Clients must recognize this early-error branch even with `--feedback` or
`--envelope`. No run ID, snapshot identity or report location is invented.
Completed report and envelope schemas are unchanged. Feedback byte limits
also bound early errors; oversized causes are omitted before recovery arguments.
If complete arguments do not fit, an instruction replaces the command.
Clap syntax failures retain their existing usage output and exit status.

Selected checks retain issues in `metadata.prerequisites`; feedback points to
the full report. Representative codes are `config_invalid`, `input_missing`,
`input_unreadable`, `git_unavailable`, `git_repository_invalid`,
`git_ref_unavailable`, `snapshot_unavailable`, `rule_assets_unavailable`,
`plan_invalid`, `tool_unavailable`, `tool_probe_failed`, `workspace_unavailable`,
`storage_unavailable`, `lock_unavailable`, `archive_unavailable`,
`evidence_invalid`, `policy_authentication_failed` and `remote_unavailable`.
Unclassified failures retain an `unknown_error` and their cause; clients must
not classify them by matching operating-system error text.
