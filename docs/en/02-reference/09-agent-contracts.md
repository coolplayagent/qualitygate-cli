# Consuming-agent contracts

[简体中文](../../zh/02-reference/09-agent-contracts.md) · [Volume index](README.md)

`agent.contracts.v1` introduces policy `agent_obligations`, exported
`schema agent-obligation`, and read-only `agent-contract` projection. These
instructions address the agent doing the work. The
[packaged reference](../../../skills/qualitygate-cli/references/agent-contracts.md)
documents the complete shape, limits, commands and adoption workflow.

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
context recovery are not provided by this protocol. Current Rust harness
integration is a subsequent stage of the architecture design.

Sources are protected from exclusions and preserved in frozen policies.
Definition/check-setting changes invalidate adoption. Source and selected
policy changes invalidate readiness. Staged/diff projection uses those selected
bytes, preserving the same policy trust boundary as normal checks.
An ordinary passing code gate does not satisfy behavior obligations by itself.
