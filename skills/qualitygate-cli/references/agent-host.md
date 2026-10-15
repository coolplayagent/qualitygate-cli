# Rust consuming-agent host and immutable delivery

Requires `agent.contracts.v1`, `agent.host.v1` and `agent.delivery.v1`. These are
development capabilities after published 0.5.7; compare executable identity and
capabilities. The [contract reference](agent-contracts.md) defines adopted
obligations and Prompt readiness. This host implements their supported controls.

Use `agent-run` when the user has selected this host workflow and authorized
the configured agent command. Loading this Skill does not itself authorize
launching another Agent. Preserve the selected repository, policy commit, task,
role and permissions. The user supplies an isolated Git checkout, an external
evidence directory, a task-instruction file (UTF-8, at most 16 KiB), and a JSON
argv file (at most 16 KiB, 1..64 arguments) for a stdin-compatible agent command.
No shell interpolation or model-provider-specific execution is implied.
This initial host requires explicit Git base and policy references. A signed
active-policy reference mode is not adapted; incompatible policy selection
remains incomplete rather than falling back to a different authority.

Run the matching executable with:

```text
qualitygate --root <isolated-checkout> agent-run
  --qualitygate <absolute-matching-qualitygate>
  --base <base-commit> --policy-ref <authorized-policy-commit>
  --task <task.yaml> --role implementer
  --prompt-file <external-task.txt> --agent-command <external-agent.json>
  --output-dir <existing-external-directory>
  --max-attempts 3 --timeout-seconds 1800 --format json
```

The command pins the evaluator identity and resolves Git references to full
commits before launching the agent. It independently runs the initial full
check, projects current obligations, verifies host support, and supplies the
complete contract plus task and feedback on stdin before each attempt. The
receipt records session, attempt/event, role/task, contract/prompt/input digests
and the actual input channel. `stdin_provided` means the host supplied those
bytes; it does not prove the model read, understood or followed them.

After each successful process exit, the host independently runs full checks
against the fixed policy/task. Agent stdout is task data, never acceptance
evidence. The controller retains the selected CLI's direct captured report in
memory; it does not accept an agent-supplied report path. The evaluator is
rehashed before/after calls. Changed policy, verification assets, checkout HEAD,
task/contract bindings, missing evidence and incomplete runs cannot authorize
delivery. Required check-backed obligations also reject warning-level failures.
Warnings and optional gaps remain in retained evidence.
For signed independent acceptance, pass both `--trust-store` and `--evidence-dir`;
the selected CLI authenticates those external inputs under the existing task
contract. Missing required acceptance remains incomplete and is never waived.

Supported host controls are `prompt_injection`, `policy_integrity`,
`final_full_check` and `immutable_delivery`, at the controller's process-start,
repair and delivery entry points. `before_tool` interception,
`tool_authorization`, `final_response` and `in_process_context_recovery` are
unavailable. A required unavailable control blocks before agent launch;
optional unsupported controls remain explicit. Free-form prohibitions still
have only instruction strength unless covered by a declared implemented check
or control. The host does not infer intent or enforce arbitrary prose.

`--resume <previous-index.json>` starts an explicitly requested new invocation
with fresh budgets and checks. Prior state is bounded context, never reusable
completion evidence. Current obligations are reloaded and reinjected. Changing
the configured agent command records a handoff; otherwise the first event is
resume. Every later attempt is a new process with a fresh repair projection.
Internal context compression within an arbitrary command is not intercepted.

Default feedback is compact; `--feedback-mode full` includes the full report.
`--replacement-file src/<existing-file>` accepts a plain JSON object containing
exactly `path` and complete UTF-8 `content`, or the existing agent-message event
format. The host confines replacement to that existing regular non-symlink file,
rejects concurrently changed input, then performs the same full verification.
The content limit is 1 MiB. The combined input prompt is at most 1 MiB.

The host allows 1..3 attempts and 1..1800 seconds per invocation, stops after
two attempts without a strictly reduced diagnostic set, and retains edits and
bounded logs on failures/timeouts. The existing separate ten-second evidence
cleanup window remains. Configured process limits and independently bounded
identity/export I/O also apply; the host never accepts after its deadline.

Completion names a content-addressed ZIP of the exact checked snapshot, with
file bytes, paths and executable modes, not the mutable workspace. The exporter
hashes its own generated byte stream, publishes without overwriting an existing
object, checks file integrity, and marks the object read-only. The final host
decision binds that object to current full report, policy, task, scope, contract
and injection receipt. Recipients **must verify the artifact digest** before
consuming it; read-only permissions do not authenticate a file or prevent
same-user replacement.

The lower-level `check --export-delivery <external-directory> --format json`
returns `{schema_version: 1, kind: delivery_check, report, delivery}`. Failing or
incomplete checks have no delivery object; quick/path-filtered checks cannot
export. This check alone does not establish Agent behavior compliance. The ZIP
contains all unexcluded captured context, including untracked files, and is
bounded by snapshot acquisition limits, 2 GiB output and 120-second export/hash
operations. The exporter never rereads live source files after checking.

The caller must protect the trusted host, evaluator and evidence from hostile
OS access. This command is not a sandbox for arbitrary agent commands, does not
authenticate model/operator identity, and cannot control side channels or
background processes that escape the configured process boundary. Claims are
limited to the documented controlled entry points and selected checks. The
compatibility `examples/agent_loop.rs` delegates to the same reusable core.
