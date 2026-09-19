# Real-repository pilot

[简体中文](../../zh/01-user-guide/05-real-repository-pilot.md) · [Volume index](README.md)

The pilot protocol measures real repository benefit separately from fixture and
CI evidence. Local tests show that the machinery behaves as specified; they do
not establish human-confirmed detection, false-positive rate, repair benefit,
or review savings.

## Current v10 contract

New pilots use `templates/pilot/observation-v10.json`. Before observations,
declare eight independent inputs (four bug fixes and four refactors), two model
groups, the existing-tools and Qualitygate workflows, an alternating run order,
task-source artifacts, an initial full report for every assignment, and bounded
attempt/time/no-progress rules. With two models and two workflows, the complete
matrix contains 32 run units.

Every attempt, including failures and timeouts, has a digest-checked model
capture and execution receipt. Requested and reported model identities remain
separate; an unknown actual route is explicit. Receipts bind harness identity,
start/end claims, status, snapshot, and report. Digests do not authenticate a
provider or clock, so independent review still needs original logs.

v10 intentionally needs no prices, currency, hourly rate, or monetary cap. Its
final subject evaluates eight nonfinancial thresholds: detection, false
positives, repair success, required-check completion, review coverage, matched
inputs, review-time reduction, and full-check P95 ratio. Older v1–v9 records
retain their versioned budget and threshold semantics.

## Evidence sequence

1. An external owner selects the real tasks, immutable source, trusted policy,
   task contracts, reviewers, and durable evidence archive.
2. `pilot seal` freezes the complete plan before observations.
3. `pilot authorization-subject` produces the exact subject that a human owner
   signs through an external DSSE/Ed25519 trust system.
4. The two workflows run under comparable conditions for the declared window;
   all attempts, omissions, incomplete checks, and reviews stay in the record.
5. `pilot acceptance-subject` evaluates the sealed evidence. A distinct
   reviewer signs acceptance or rejection.
6. `pilot summarize` verifies the supplied records without changing policy or
   manufacturing absent measurements.

Missing tasks, reviewer assignments, external trust, durable archives, initial
reports, observations, or signatures mean the pilot has not started or has not
completed. Use null plus a reason for absent data; never replace missing values
with zero. Historical replay and controlled fixtures remain engineering
evidence outside the real-pilot denominator.
