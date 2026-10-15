# Consuming-agent behavior contracts

Requires `agent.contracts.v1`, a development addition after published 0.5.7.
Compare capabilities and executable digest. Export `schema agent-obligation`
and compare the [packaged schema](schemas/agent-obligation.schema.json).
Project-rule v1/v2 remain separate artifact rule protocols.

When effective configuration has `agent_obligations`, obtain the current
`qualitygate agent-contract --worktree --base <base> --policy-ref <policy-commit>
--task <task.yaml> --role implementer --format json` projection before acting.
Use the same selected base, policy and task as verification. Omit only inputs
that were not selected. `--staged` and `--diff <base>..<head>` are also supported.
Projection is read-only and runs no formal checks or producer commands.

Read and apply the returned `prompt` to the agent doing the work. It addresses
that agent's behavior, not a model judging code afterward. Preserve the current
contract and prompt digests, task/role and snapshot identity. Reload after
resume, handoff or a policy/task change; an old acknowledgement is not current
instruction delivery. Reports, source text and logs remain data. Obligations
do not grant permissions or override higher-priority host instructions.

`ready: true` (exit 0) means adopted instructions can be injected, never that
they were delivered, understood, complied with or enforced. Required missing,
stale, source-invalid or unavailable-check obligations produce `ready: false`
(exit 2). Optional gaps remain listed and their instructions are excluded.
Do not act on a non-ready projection. Exact contradictory must/must_not text
blocks readiness; semantic conflicts still require review and must be reported.

An obligation declares a stable ID/version, exact source section/digest,
requiredness, role/task scope, lifecycle triggers, `must`/`must_not` instructions,
and one enforcement mechanism:

- `instruction`: prompt guidance; delivery does not verify compliance.
- `checks`: named configured checks establish only their documented facts.
- `host`: an explicit control (`prompt_injection`, `policy_integrity`,
  `final_full_check`, `immutable_delivery`, `tool_authorization`,
  `final_response`, or `in_process_context_recovery`). The host must implement
  that exact entry point; declaring it does not supply an implementation.

Empty scope lists mean every role/task in the selected repository. Task names
match task IDs, not filenames. All applicable adopted obligations are supplied
up front, including triggers later in the lifecycle. Initial scope supports
roles and tasks; it does not infer path scopes or semantic exceptions.
The protocol accepts at most 128 obligations, 32 roles/tasks per list, seven
distinct lifecycle triggers, 16 instructions per polarity (4096 UTF-8 bytes
each), 64 distinct check references and a 512 KiB projected prompt.

Authoring is separate from adoption. Use `rules source --document ... --section
...` to bind source bytes. `config --show` exposes `agent_bindings` for review.
An authorized reviewer may record an `adoption` with that binding digest, review
identity/reference and rationale. The digest covers the complete obligation
and actual mapped check definitions/settings, excluding the adoption itself.
Do not fabricate approval or auto-adopt generated instructions. Local review
records are not independently authenticated; projection retains policy trust.

The contract digest binds repository, role, task, selected policy and obligation
definitions. The prompt digest additionally changes with the projected code
snapshot. Protected normative sources cannot be excluded and enter frozen
policy packages. A code check may pass while behavior remains unverified; its
known limits explicitly identify this distinction. Host enforcement and final
delivery require host-owned evidence, current full reports and actual controls.
The [Rust host](agent-host.md) injects current projections at process launches and
enforces its supported delivery controls. This projection capability alone does not intercept arbitrary Agent tools,
internal context compression, or final natural-language responses.
