# Pilot evidence summaries

For a new pilot, start from the bundled
[v7 manifest template](../assets/pilot/observation-v7.json), fill the complete
protocol and assignment matrix, then run `qualitygate pilot seal --input <plan.json>
--format json` before collecting outcomes. Retain that stdout as the observation
manifest in the declared external archive. Sealing rejects existing observations,
incomplete governance, non-real inputs, missing ground truth and unbalanced
model/workflow cohorts.

Manifest schema v3 requires a predeclared `protocol.task_mix`. For this pilot,
use four independent `bug_fix` and four independent `refactor` inputs. The
counts must sum to `task_count` and match distinct input IDs in the assignment
matrix. Distinct inputs cannot reuse a task ID or task-contract digest. The
summary's `sampling_audit` lists the declared and observed counts and one task
identity per input. Review real issue/commit sources and sampling records before
sealing; distinct strings and digests cannot prove semantic independence.

Manifest schema v4 also requires one `sources` entry for each independent input.
Declare an issue or commit source ID, the archive-relative path, exact byte
length, SHA-256 digest and selection time no later than sealing. Keep each
regular source file beside the external observation manifest (at most 64 KiB
each and 8 MiB total). The CLI checks confined paths and bytes at seal, start
subject, summary and final subject calls; missing or changed files are incomplete
evidence. `source_audit` shows one source per input. Human reviewers must still
confirm that the files match real, distinct issue/commit records and that the
selection time is credible.

Manifest schema v5 also requires `run_order`: every assignment ID exactly once,
adjacent workflows alternating, with existing-tools-first and qualitygate-first
counts balanced within each task-kind and requested-model stratum. Record each
actual run's `start_sequence` (1-based) in its observation. The summary's
`schedule_audit` reports missing positions and deviations; either blocks final
acceptance. Do not infer start order from completion timestamps. Compare the
declared positions with retained external start-event logs.

Manifest schema v6 additionally requires a digest-bound initial full report for
every assignment and a sealed `protocol.no_progress_limit` (2 for this pilot).
The CLI verifies each baseline report against the initial snapshot, task,
policy, environment, tools and required checks, then rereads its archived bytes
for seal, start subject, summary and final subject calls. Initial and attempt
reports share a 64 MiB read budget. `attempt_audit` recomputes progress only
when the set of error diagnostic identities and gate blockers strictly shrinks
in a complete bound full report. Failed, timed-out or abandoned attempts with
no report count as no verified progress. Runs past the sealed attempt/time cap
or after two consecutive nonprogress attempts block final acceptance while
remaining in cost and failure denominators. Review external Agent/clock logs
to establish the truth of declared durations and outcomes.

Manifest schema v7 adds `model_evidence` to each observation. Archive one JSON
capture per run and declare its confined relative path, SHA-256 and byte length
alongside the same structured capture in the manifest. Bind assignment ID,
Agent version, harness digest, requested model, reasoning effort and capture
time. Use `status: unknown`, `actual_model: null` and a nonempty
`unknown_reason` when the provider does not expose a routed model. Use
`status: reported` with `actual_model` and no unknown reason only when an
actual model is reported. The capture must match the assignment's post-run
`cohort.actual_model`; the requested fields must match the sealed plan. Missing
captures make evidence incomplete. An explicit unknown actual model does not
block v7 protocol readiness; comparisons are about requested configurations.
Review the provider's original logs and clock independently: an archived
capture is a declaration, not authenticated provider routing.

For an unknown routed model, write the `capture` object below as a JSON file
in the external pilot archive. Put its exact relative path, SHA-256 and byte
length in `model_evidence.artifact`, and put the same parsed object in
`model_evidence.capture`:

```json
{
  "assignment_id": "run-01",
  "captured_at": 1780000000,
  "agent_version": "recorded-agent-version",
  "harness_digest": "sha256:replace-with-real-digest",
  "requested_model": "requested-model",
  "reasoning_effort": "medium",
  "actual_model": null,
  "status": "unknown",
  "unknown_reason": "Provider response did not expose a routed model identifier"
}
```

The example values are placeholders, not valid trial evidence. Capture time
must be within the sealed window and no later than `observed_at`. The CLI
limits each model file to 64 KiB and the model inventory to 8 MiB. For a
reported model, set `status` to `reported`, use the provider's model string in
`actual_model`, and set `unknown_reason` to null. A reviewer still needs the
provider's original record to assess that claim.

Manifest schema v2+ requires a structured `protocol.budget`: three-letter
uppercase currency, price timestamp, rate-card source, total cap in currency
microunits, and human hourly rate in microunits. Set legacy `monetary_cap` to
null. The cap covers all model and infrastructure attempts in both workflows,
including failed attempts, plus each observation's active human review time.
Missing costs, time, or incompatible currency/price are unknown; known spend
above the cap fails even if other inputs are unknown. Budget fields are sealed
before observation. Already sealed v1 records retain their original nine-check
assessment and human interpretation of the text cap.

Before collecting outcomes, run `qualitygate pilot authorization-subject
--input <observations.json> --format json`. Have the configured human owner sign
the exact subject as a DSSE Ed25519 record with payload type
`application/vnd.qualitygate.pilot-plan-authorization.v1+json`. The trust key ID
must equal the protocol owner and allow repository checks with the
`pilot-plan-authorization` scope. Keep the trust store and signed envelope
outside the checked repository; do not ask the CLI or an Agent to create a key.

Use `qualitygate pilot summarize --input <observations.json> --trust-store
<external-trust.json> --authorization <external-start.dsse.json> --format json`
for authenticated aggregation. Omitting both external inputs remains a read-only
descriptive replay with `plan_authorization.status=absent` and
`protocol_ready=false`. Keep report paths relative to the manifest's external archive.

The command verifies report bytes and their task, policy, snapshot, environment,
tool and required-check bindings. It retains missing, failed, timed-out and
abandoned assignments in denominators. Null metrics mean that evidence or a
denominator is missing; do not replace them with zero. Different model,
permission, tool, environment, cache, origin, task type and workflow declarations
remain separate groups.

The embedded plan digest detects later changes to the protocol and assignments;
actual routed model identities and observations may be added after sealing.
The owner signature authenticates that exact subject and is checked for scope,
validity and revocation on each summary; it is not a trusted timestamp or final
trial acceptance. The output is descriptive. Reviewer identities, canonical
issue truth, prices and authority are caller declarations. Do not treat a
summary or met threshold as signed trial approval, and do not infer causal
benefit from groups without the same declared input inventory and conditions.

After the observation window, use `pilot acceptance-subject` with the same
external trust store and authenticated start record. Review its nine required
checks: aggregate Qualitygate detection, false-positive, repair, completion and
review fractions, plus all matched inventory, review-time, full-P95 and cost
comparisons. For v2+, a tenth total-budget check is required. Failed or unknown
checks cannot be signed as accepted.

The independent human reviewer key must equal the protocol reviewer and have
the `pilot-trial-acceptance` repository scope. Sign the exact subject with
payload type `application/vnd.qualitygate.pilot-trial-acceptance.v1+json`, then
pass the external envelope with `pilot summarize --acceptance`. An authenticated
rejection is a blocking result. A signature authenticates the decision over the
retained evidence; it does not establish causal benefit or authorize automatic
rollout. Older sealed manifests keep their versioned validation and acceptance
semantics; use this bundled reference for a new v7 pilot.
