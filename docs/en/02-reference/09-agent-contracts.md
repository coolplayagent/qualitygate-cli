# Consuming-agent contracts

[简体中文](../../zh/02-reference/09-agent-contracts.md) · [Volume index](README.md)

`agent.contracts.v1` introduces policy `agent_obligations`, exported
`schema agent-obligation`, and read-only `agent-contract` projection. These
instructions address the agent doing the work. The
[packaged reference](../../../skills/qualitygate-cli/references/agent-contracts.md)
documents the complete shape, limits, commands and adoption workflow.

## Obligations and readiness

Each obligation binds a reviewed normative source and interpretation to a
role/task scope, lifecycle events, required/prohibited actions and explicit
instruction, check or host enforcement. The projection carries selected
snapshot/policy/task identity, the contract digest, prompt and prompt digest.
Only applicable, adopted, source-current obligations enter that prompt.
Missing or stale required evidence leaves readiness incomplete; optional gaps
remain visible. Exact instruction contradictions block readiness.

The CLI does not execute formal checks while projecting. A ready projection
means suitable for injection, not delivery, understanding or compliance.
Check references retain their fact boundaries. Host control declarations
require an actual host implementation; arbitrary tool interception and internal
context recovery are not provided by this protocol. The [Rust host](10-agent-host-and-delivery.md)
now injects these projections and enforces its documented controls.

## Invalidation

Sources are protected from exclusions and preserved in frozen policies.
Definition/check-setting changes invalidate adoption. Source and selected
policy changes invalidate readiness. Staged/diff projection uses those selected
bytes, preserving the same policy trust boundary as normal checks.
An ordinary passing code gate does not satisfy behavior obligations by itself.


## Obtain and consume a projection

```bash
qualitygate capabilities --format json
qualitygate schema agent-obligation --format json
qualitygate agent-contract --worktree --role implementer --format json
```

Require `agent.contracts.v1`; do not infer it from the version string. Add the
same `--base`, `--policy-ref` and `--task` inputs used for verification when
selected. Staged and diff projections are also supported. Read the returned
`prompt`, retain contract/prompt digests and inject it before the Agent acts.
Reload after a resume, handoff or relevant input change.

| Enforcement | Evidence boundary |
| --- | --- |
| `instruction` | Supplies behavior guidance; delivery alone does not prove compliance. |
| `checks` | References configured checks and only the facts they establish. |
| `host` | Names a control that the selected host must actually implement. |

Exit `0` with `ready: true` permits injection. Required gaps or exact instruction
conflicts return `ready: false` and exit `2`. Empty role/task scopes match every
role/task; task scope matches task IDs, not filenames. The protocol permits up
to 128 obligations, 32 roles/tasks per list, 16 instructions per polarity,
64 check references and a 512 KiB projected prompt.

Authoring uses exact source bindings from `rules source`. `config --show`
exposes `agent_bindings`; an authorized reviewer records adoption separately.
Changing obligations or their check settings invalidates that binding. An
adoption record cannot grant Agent permissions or override host instructions.

---

[Previous: Constraint coverage](07-constraint-coverage.md) · [Contents](../README.md) · [Next: Agent host and immutable delivery](10-agent-host-and-delivery.md)
