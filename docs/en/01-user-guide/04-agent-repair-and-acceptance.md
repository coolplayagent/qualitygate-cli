# Agent repair and acceptance

[简体中文](../../zh/01-user-guide/04-agent-repair-and-acceptance.md) · [Volume index](README.md)

Use Qualitygate feedback to give an Agent specific findings and a reproducible
recheck. Keep the repository policy, task acceptance and retry budget explicit
throughout the repair cycle.

## Obtain repair feedback

```bash
qualitygate check --worktree --profile quick --feedback --format json
```

The bounded feedback includes diagnostic IDs, locations, evidence references,
limits, recheck arguments, pending delivery checks and the original report
identity. Keep that report alongside the feedback. A quick result guides editing;
it does not authorize delivery or policy changes.

If policy declares Agent behavior obligations, first obtain and apply the current
[Agent contract](../02-reference/09-agent-contracts.md) for the selected role and
task. Reload it after a resume, handoff or relevant input change.

## Run a finite repair loop

1. Retain the initial snapshot, selected policy, task and full report.
2. Supply relevant feedback and the permitted source scope to the Agent.
3. Record each attempt, including failures and timeouts.
4. Recheck edits with the same policy and task authority.
5. Stop when successful, out of attempts or time, repeatedly making no progress,
   or unable to restore a prerequisite.
6. Verify the actual final combined snapshot with an unfiltered full check.

The harness owns attempt and elapsed-time budgets. Repairs cannot disable
required rules, lower severity, replace trusted policy or invent manual approval.
A changed snapshot always needs a new report.

| Result | Response |
| --- | --- |
| Blocking violation | Repair within the permitted source scope, then recheck. |
| Incomplete execution | Restore the missing tool, input or evidence; retain the gap if unavailable. |
| Complete full pass | Verify scope, bindings and independent acceptance before delivery. |
| Budget exhausted or no progress | Stop and report the remaining findings and incomplete items. |

## Finish task acceptance

```bash
qualitygate check --worktree --profile full --task task.yaml --format json
```

Use the task option when a task contract exists; retain any selected policy and
trust inputs on every recheck. A task adds observable acceptance items to
repository policy. Command acceptance must prove the intended test ran; zero
selected tests are not a pass. Required manual acceptance needs external signed
evidence as described in [policy, task and signed evidence](../02-reference/04-policy-task-and-signed-evidence.md).

For a passing delivery report, require `profile: full`, `scope: delivery` or
`task`, empty `plan.pending_delivery_checks`, `gate.complete: true` and
`gate.decision: pass`. Preserve warnings and known limits.

## Use the Rust host when you need controlled delivery

The reusable [`agent-run` host](../02-reference/10-agent-host-and-delivery.md)
injects current obligations, runs independent full checks and authorizes an
immutable delivery artifact. Select a runtime advertising its required
capabilities and configure the Agent command explicitly. The host reference
covers setup and the controls it supports; an instruction receipt alone does
not prove compliance or control all Agent actions.

---

[Previous: Schema-guided repository rules](06-schema-guided-repository-rules.md) · [Contents](../README.md) · [Next: Evaluating adoption in a real repository](05-real-repository-pilot.md)
