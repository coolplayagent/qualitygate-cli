# Policy, task, and signed evidence

[简体中文](../../zh/02-reference/04-policy-task-and-signed-evidence.md) · [Volume index](README.md)

Repository policy, task acceptance, and external trust are distinct authorities.
The selected policy defines required checks and severities. A task contract adds
acceptance items for one request. A trust store and signed records authenticate
specific human or system declarations. None may silently weaken another.

## Policy lifecycle

A candidate revision records its parent, source inputs, immutable artifacts,
validation matrix, and observed differences. Independent paired validation uses
the same base/head Git revisions and permits before the candidate can be promoted.
Each policy captures its own exclusions; differing execution inputs bind both
snapshot identities instead of sharing one policy's filtered content.
Approval and rollback are separate signed subjects. Active policy history is
append-only and auditable; a local branch name or digest alone is not approval.

The protected suite's optional `budget.snapshot_max_file_mib` defaults to 2 and
accepts integers 1–8. It applies equally to baseline/candidate captures and test
overlays. It is bound by the externally authorized suite digest, so capacity
changes require updated external authorization, not a candidate-policy edit.

Diagnostic trends, oracle review, gate execution, downstream benefit, and
maintenance cost are separate observations. Missing denominators and absent
measurements remain unknown. Policy evolution must not convert warnings or
incomplete evidence into pass merely to improve a metric.

## Task plans and rechecks

A task file contains a stable task ID, acceptance IDs, observable behavior,
verification kind, related check IDs, requiredness, and severity. Its digest is
reported with the selected policy and snapshot. Command verification retains
the exact invocation and proves the expected test/report was produced.

Recheck context binds the original report, selector, policy reference, task,
resource options, and evidence inputs. A changed final snapshot always requires
a new report. `quick`, path-scoped, or pending-delivery results cannot fulfill a
full acceptance item.

## Signed records and provenance

Manual acceptance and pilot authorization/acceptance use caller-owned DSSE and
Ed25519 trust inputs outside the checked repository. Verification checks exact
subject bytes, key scope, role, validity, maximum age, revocation, and binding
to snapshot/policy/task. Missing, stale, foreign, expired, revoked, or malformed
records are incomplete.

`snapshot.content_digest` identifies captured execution files. When present,
`snapshot.verification_digest` additionally binds scope, exclusions, changed
ranges and the path filter, and is required in signed snapshot bindings. The
same file contents do not make delivery and repository evidence interchangeable.

Agent provenance authenticates declared inputs, outputs, transformations, and
tool identity. Git trailers bind declarations to commits and reject ambiguous
or missing associations. These mechanisms establish provenance, not semantic
correctness or independent review.

Versioned decision and feedback envelopes preserve the original deterministic
payload plus evidence summaries, warnings, gaps, pending work, and omissions.
An optional external judgment provider may assess warnings in shadow or advisory
mode. Probabilistic, abstained, invalid, and execution-gap outcomes never change
the deterministic gate or approve policy.
