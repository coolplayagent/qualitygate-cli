# Agent host and immutable delivery

[简体中文](../../zh/02-reference/10-agent-host-and-delivery.md) · [Volume index](README.md)

`agent.host.v1` exposes the reusable Rust controller through `agent-run`.
`examples/agent_loop.rs` delegates to that same implementation. Any configured
stdin-compatible agent command can receive the current adopted contract,
task instructions and feedback before each bounded attempt. The
[packaged host reference](../../../skills/qualitygate-cli/references/agent-host.md)
defines arguments, receipts, limits, recovery and trust assumptions.

## Verification and delivery

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
The output directory must stay outside the canonical repository root; equivalent
path spellings and Windows path prefixes cannot bypass that boundary.

## Supported controls

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


## Configure the host

Require `agent.contracts.v1`, `agent.host.v1` and `agent.delivery.v1` from the
selected runtime. Prepare an isolated checkout, an existing evidence directory
outside it, a task instruction file, and a JSON argv array for a command that
accepts instructions on stdin. Both input files are UTF-8 and at most 16 KiB;
the argv array contains 1–64 arguments. No shell interpolation is implied.

The following is an argument template; replace the placeholders before execution:

```text
qualitygate --root <isolated-checkout> agent-run
  --qualitygate <absolute-matching-qualitygate>
  --base <base-commit> --policy-ref <authorized-policy-commit>
  --task <task.yaml> --role implementer
  --prompt-file <external-task.txt> --agent-command <external-agent.json>
  --output-dir <existing-external-directory>
  --max-attempts 3 --timeout-seconds 1800 --format json
```

The host requires explicit base and authorized policy commits; signed active-policy
selection is not adapted. For independent acceptance, supply `--trust-store` and
`--evidence-dir` together. It allows 1–3 attempts and 1–1800 seconds per invocation,
and stops after two attempts without a strictly reduced diagnostic set.

`--resume <previous-index.json>` begins a new invocation with fresh budgets and
checks. A changed Agent command records a handoff. Neither uses old completion
evidence. `stdin_provided` records supplied bytes, not model understanding.

A lower-level `check --export-delivery` returns an envelope with `report` and
`delivery`; failed or incomplete checks have no delivery object. Quick or
path-filtered checks cannot export. The ZIP is bounded by snapshot limits,
2 GiB output and 120-second export/hash operations. Consumers verify the digest
before use; subsequent live workspace edits are outside that artifact.

---

[Previous: Consuming-agent contracts](09-agent-contracts.md) · [Contents](../README.md) · [Next: Logical view](../03-architecture/01-logical-view.md)
