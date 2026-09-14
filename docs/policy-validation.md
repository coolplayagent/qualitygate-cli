# Protected validation, approval and inheritance

See [policy evolution](policy-evolution.md) for evidence, immutable archive
formats and requirement tracking. Creating the archive enables governance:
subsequent semantic edits use `policy candidate rules`. Before an archive
exists, legacy YAML commands remain untrusted draft-authoring operations.
Category metadata remains mutable in either mode.

## Independent paired validation

```bash
qualitygate policy evaluator --format json
qualitygate policy candidate validate <candidate-id> --baseline <parent-policy-digest> --task /trusted/suite.json --trust-store /trusted/trust.json --evidence-dir /trusted/runs --jobs 4 --actor evaluator-operator --actor-kind human --format json
qualitygate policy evaluation <evaluation-digest> --format json
qualitygate policy record <attempt-digest> --kind validation_attempt --format json
qualitygate policy record <report-digest> --kind check_report --format json
qualitygate policy record <artifact-digest> --kind blob --format json
```

The operator supplies strict JSON/YAML inputs outside the checked repository.
Suite and trust files are each at most 1 MiB, regular, and without symlink
ancestors. Their exact bytes and file identities are rechecked at completion;
on Unix, ctime/inode checks reject modified-and-restored bytes as well. The
external evidence directory must already exist. Once execution begins, an
immutable attempt is retained before any child runs. Pass, block and incomplete
remain distinct. `policy candidate abandon` freezes unfinished work and
invalidates an in-flight validator's final publication.

The strict `ValidationSuite` fields are `schema_version: 1`, `id`,
`baseline_policy`, `evaluator_epoch`, `motivating_evidence`, `budget`,
`min_improvements`, `max_rules`, `max_rule_growth` and `cases`. The baseline
must equal the candidate's immutable parent. There are 3..64 cases with unique
IDs, snapshot pairs and task IDs, including replay, held-out and anchor cases.
Each case contains `id`, `kind: replay|held_out|anchor`, full lowercase Git
`base`/`head` commit IDs, a complete existing `TaskContract` in `task`, and
`expectations`: a map from check IDs to `pass|fail|skipped|absent`.
Every executed check needs an independent expectation. An intentional failing
check may satisfy a negative-case oracle; incomplete execution cannot.

The strict `EvolutionTrust` fields are `schema_version: 1`, canonical absolute
`repository`, SHA-256 allowlists `suites`, `baselines`, `evaluators`,
`approval_keys`, `revoked_approvals` and `max_age_seconds`. Each key has `id`,
`actor: {id, kind: human}` and base64 Ed25519 `public_key`. Duplicate and weak
keys are rejected. The suite allowlist covers exact file bytes. The evaluator
digest covers the running executable; `policy evaluator` exposes it for
independent authorization. All inherited environment entries contribute to a
framed SHA-256 digest; report output does not disclose their values.

Both policies use the same captured source, task, evaluator and environment,
including matching executable digests and declared tool-version/input evidence
for checks shared by the pair,
with separate execution directories. Each candidate expectation must match,
and at least `min_improvements` cases must reduce the parent's mismatch count.
The configured-rule count and growth must satisfy the protected contribution
limits, each bounded at 512. Violations produce block only after complete
execution; a missing tool or timeout remains incomplete. Semantic edits clear
validation and approval references. A replacement evaluator needs a separately
authorized epoch and independent anchor cases; scores are not pooled across
epochs. The executable is hashed again at validation completion.

Paired validation executes configured local commands, built-in/project rules
and supported compatibility checks. It cannot manufacture external manual or
provenance records. Missing required external evidence remains incomplete.
Evaluation covers the selected tasks and snapshots, not future production use.

## Resource and scheduling contract

The suite's `budget` has these strict fields:

| Field | Bound and meaning |
|---|---|
| `snapshot_max_mib` | 1..1024 MiB per existing snapshot acquisition budget |
| `snapshot_jobs` | 1..16 readers per scheduled case |
| `snapshot_timeout_seconds` | 1..3600 seconds for acquisition |
| `max_live_snapshot_mib` | At most 4096 MiB, at least twice `snapshot_max_mib` |
| `max_parallel` | 1..8 global concurrently executing policy checks |
| `case_timeout_seconds` | 1..3600 seconds, including capture and waiting for check permits |
| `total_timeout_seconds` | 1..7200 seconds for case scheduling and execution |

`--jobs` can reduce the maximum. Live case count is at most
`min(jobs, max_live_snapshot_mib / (2 * snapshot_max_mib))`. This reserves both
snapshot trees before a case starts; it is a bound on captured source contents,
not total process RSS. Snapshot reader concurrency is additionally bounded by
the live case count times `snapshot_jobs`. Baseline and candidate share one
`Arc<Snapshot>`; materialization and input guards borrow its contents rather
than duplicating the entire repository in memory. Only scheduled checks get
execution directories. Results are sorted deterministically.

Completed full reports are archived pair by pair instead of retaining all
case reports in memory. Execution artifacts and declared tool-probe logs are
copied into content-addressed objects after verifying their digest and byte
count, with at most 4,096 distinct artifacts per report. Approval verifies these retained bytes, so external run-directory cleanup
does not erase the execution evidence. `policy record --kind blob` returns
bounded base64 bytes. Individual serialized archive records are limited to
8 MiB during serialization. Process time/output limits and process-group cleanup
remain enforced by the runner. Timeouts stop scheduling and mark uncompleted
cases incomplete. Acquisition, blocking filesystem work and serialization have
their own cooperative deadlines; setup and final archive publication are
outside the case-execution timer. The evaluator identity streams 64 KiB chunks
with a 30-second deadline and a separate 1 GiB limit for debug executables;
configured producer executable identification retains its 256 MiB limit.

These are cooperating-process protections, not an operating-system sandbox.
Operators control external trust and approvals and must preserve archive
durability. The CLI does not stop a privileged actor from rewriting arbitrary
files or replacing an entire historical archive.

## Signed approval and activation

```bash
qualitygate policy candidate approval-subject <candidate-id> --trust-store /trusted/trust.json --format json
# An independent operator signs the returned subject outside qualitygate.
qualitygate policy candidate approve <candidate-id> --approval /trusted/approval.json --trust-store /trusted/trust.json --format json
qualitygate policy candidate promote <candidate-id> --format json
```

The DSSE payload type is
`application/vnd.qualitygate.policy-approval.v1+json`. The strict payload has
`schema_version: 1`, the exact returned `subject`, `approver`,
`decision: approved|rejected`, Unix `issued_at` and `expires_at`, and `reason`.
The subject binds repository, candidate ID/revision/package, immutable parent,
evaluation, suite, trust and evaluator digests, and generation identity.
Approval reopens retained reports and recomputes plan bindings, completeness
and oracle comparisons. The signer must be a trusted human distinct from the
generating actor. There is no signing or private-key-generation command.

Promotion rechecks live trust, signature, expiry, revocations, evaluator and
active parent before atomic selection. The prior active candidate becomes
superseded; YAML and older objects are not overwritten. `config --show`, rule
discovery and checks use the frozen definitions/settings. Only navigation
metadata comes from the draft. An explicit task must be retained in the active
package's `verification_assets`; all retained non-configuration assets must
match the checked snapshot. `--policy-ref` cannot override an active package
with another Git policy. Protected candidate validation provides historical
comparisons.

Active authorization is checked against live trust, including revoked records
and removed keys. Age is evaluated at activation time; ordinary passage of an
approval's expiry does not silently disable its already activated version.
Checks require the authorized evaluator and revalidate trust and active
selection before completion. Every real report binds snapshot, policy, task
(when supplied), evaluator and environment identities and its known limits.

## Auditable rollback

```bash
qualitygate policy rollback-subject --to <previous-policy-digest> --trust-store /trusted/trust.json --actor proposer --format json
# The independent operator signs this distinct rollback subject.
qualitygate policy rollback --to <previous-policy-digest> --trust-store /trusted/trust.json --approval /trusted/rollback.json --format json
```

Rollback uses DSSE type `application/vnd.qualitygate.policy-rollback.v1+json`
and the same outer payload fields. Its subject binds current policy,
authorization and activation time, history cursor, target history record and
digest, proposer, repository, trust and evaluator. The target must have appeared
in a promotion/rollback transition, including the original parent. Independent
human approval is required; an unrelated draft cannot be trusted by rollback.
Any history change invalidates the signed subject. The new transition retains
reason and signer, marks the abandoned active candidate rolled back, and
retains all historical definitions. The signature cannot be replayed against
the resulting state. Live revocations also apply to active rollback authority.

## Lifecycle and longitudinal observations

```bash
qualitygate rules demote line-ending --candidate <candidate-id> --actor maintainer --reason "Review false positives" --format json
qualitygate rules deprecate line-ending --candidate <candidate-id> --actor maintainer --reason "Replacement proposed" --format json
qualitygate rules retire line-ending --candidate <candidate-id> --actor maintainer --reason "No longer selected" --format json
qualitygate rules revoke line-ending --candidate <candidate-id> --actor maintainer --reason "Evidence invalidated" --format json
qualitygate rules revalidate line-ending --candidate <candidate-id> --actor maintainer --reason "New independent cases" --format json
qualitygate rules history line-ending --limit 32 --format json
qualitygate policy effectiveness --policy-ref <policy-digest> --offset 0 --limit 32 --format json
```

Each lifecycle edit targets a separate candidate and records its evidence,
actor, reason and time. Demotion selects warning severity and optional checks.
Deprecation adds metadata and retains settings. Retirement/revocation exclude
the rule from future plans after promotion; definitions remain replayable.
Revalidation enables the candidate rule with current parameters/severity for
a fresh protected run. None of these commands grants approval.

Effectiveness pages separate update validity, selected/applicable rules,
completed gate checks, findings, paired oracle improvement and observed runtime.
Downstream benefit, subsequent agent use, context tokens, review effort and
false-positive counts remain unknown without independent later observations.
Raw findings are not labeled false positives. `rules history` (alias `evidence`)
exposes retained source claims and explicit candidate lifecycle states.

Longitudinal groups require identical parent, suite, evaluator epoch/digest,
environment, resource budget and actual job count. They retain timestamped
observations, including rejected/incomplete variants. A page is not a complete
archive aggregate. Serial/parallel benchmarks compare decisions under the same
maximum budget while recording different job counts; their costs stay in
separate effectiveness groups.
