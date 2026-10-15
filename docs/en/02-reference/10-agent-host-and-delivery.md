# Agent host and immutable delivery

[简体中文](../../zh/02-reference/10-agent-host-and-delivery.md) · [Volume index](README.md)

`agent.host.v1` exposes the reusable Rust controller through `agent-run`.
`examples/agent_loop.rs` delegates to that same implementation. Any configured
stdin-compatible agent command can receive the current adopted contract,
task instructions and feedback before each bounded attempt. The
[packaged host reference](../../../skills/qualitygate-cli/references/agent-host.md)
defines arguments, receipts, limits, recovery and trust assumptions.

The host pins the selected CLI, base and policy commit, obtains full task
checks independently, and supplies current Prompt bytes. Repair, explicit
resume and agent handoff reload and reinject obligations. Prior completion or
Agent-supplied reports cannot replace fresh captured evidence. Required missing
host controls block before launch. Full checks and source/policy/task/scope
bindings guard the controlled delivery transition.

`agent.delivery.v1` adds `check --export-delivery <external-directory>`. Only
passing full unfiltered checks export a content-addressed ZIP from the same
in-memory snapshot used for verification. It includes unexcluded context,
untracked files and executable modes. The digest is computed over the generated
stream, never an Agent-provided file. Later workspace changes cannot enter it.
The host checks the object's integrity and authorizes that digest, not live
workspace contents. Recipients must verify the digest before consumption.

Supported controls are Prompt injection, fixed-policy integrity, final full
checking and immutable delivery at this controller's entry points. Arbitrary
tool authorization, final natural-language responses and in-process context
recovery remain unsupported. Source/Prompt receipt proves supplied bytes, not
model understanding or instruction compliance. Required check-backed obligations
retain violations even at warning severity; other warnings remain in evidence.

The caller must isolate trusted execution/evidence from hostile OS access.
Read-only objects are tamper-evident by digest, not authenticated storage.
This host is not an OS sandbox, does not authenticate the model or operator,
and does not imply universal interception of arbitrary agent behavior.
