# Evidence-gated policy evolution

Implementation contract for [issue #4](https://github.com/coolplayagent/qualitygate-cli/issues/4).
The current-artifact loop repairs code against protected acceptance criteria.
The policy loop retains evidence, creates a separate candidate, compares it
against its immutable parent, and requires independent authorization before
activation. Passing a weaker candidate on its motivating failure is insufficient.

## Requirements and completion evidence

This table tracks the complete issue scope and its regression targets.
The verification section distinguishes completed runs from newly added tests.

| Requirement | Required evidence | Status |
|---|---|---|
| A1: runtime category CRUD and strict JSON/YAML contracts | CLI create/rename/delete, invalid input rejection | `rule_management`, `policy_categories` |
| A2: many-to-many membership independent of enforcement and package origin | Multiple memberships survive rename/delete; settings/reviews unchanged | `policy_categories::memberships_are_additive_and_category_changes_preserve_enforcement` |
| A3: filtered/context reads preserve mandatory baseline | Category, language and source filters cannot omit mandatory rules/checks | `policy_categories::mandatory_context_survives_filters_and_local_policy_weakening` |
| B1: bounded evidence with source digests, scope, claims, sensitivity and limitations | Round-trip, capability-gap, corrupt/oversized/unknown-field rejection | `policy_candidates`, `domain::evolution::tests`, `config::policy_store::tests` |
| B2: semantic changes produce evidence-linked candidate revisions | Immutable parent, actor/reason/patch, atomic conflict handling | `policy_candidates`, `policy_lifecycle`; governed archives reject legacy semantic writes |
| B3: immutable active versions and queryable candidate/history records | Tamper detection, concurrent/crashed writer, immutable version tests | `policy_candidates`, `policy_store`, `policy_promotion` |
| B4: paired replay and independent held-out validation | Identical pinned inputs/tasks/environment/budget; preserve pass/block/incomplete | `policy_validation`, `domain::policy_evaluation::tests` |
| C1: protected acceptance and distinct generation/approval identities | External trust and signed approval; reject self-approval, stale/mismatched/revoked records | `policy_promotion`, `policy_validation` |
| C2: versioned promotion and rollback | Complete validation required, immutable history, auditable rollback | `policy_promotion`, including rollback signatures, stale transitions and live revocation |
| C3: report binding and experience archive | Snapshot/policy/task/evaluator/environment digests; logs, rejected/abandoned attempts and limits queryable | `policy_validation`, `policy_candidates`, `config::policy_artifacts::tests`; promotion survives external log-directory deletion |
| D1: revalidate/demote/deprecate/retire/revoke | Candidate-only semantic lifecycle changes; retain historical replay | `policy_lifecycle` |
| D2: selective retention and library growth control | Protected contribution/size limit rejects unbounded accumulation | `policy_validation::protected_library_growth_blocks_complete_execution_and_abandoned_attempts_stay_archived`, `config::policy_store::tests` |
| D3: longitudinal effectiveness | Separate validity/activation/use/benefit/cost, compare only matched budgets and evaluator epochs | `policy_validation`, `domain::policy_evaluation::tests` |
| Performance: bounded parallel evaluation for large repositories | Serial/parallel equality, actual worker overlap, memory/job/deadline bounds, 18,000-file fixture | `policy_performance`, native barrier/timeout cases in `policy_validation`, existing snapshot/catalog gates |

## Available archive commands

The shipped [selfcheck corpus](selfcheck.md) adds 85 independently asserted
policy fixtures, runnable without a repository. Its requirement mapping is:

| Requirements | Fixture IDs (prefix `evolution-`) |
|---|---|
| A2–A3 | `context-*`, `category-*` |
| B1–B3 | `candidate-*`, `lifecycle-*` |
| B4, C1 | `negative-oracle`, `held-out-regression`, `anchor-regression`, `missing-*`, `pending-delivery`, `timeout-dominates-regression`, `protected-input-change`, `producer-*`, `suite-*` |
| C1–C2 | `approval-*`, `rollback-*`, `workflow-promote-*`, `workflow-rollback-2`, `workflow-stale-rollback-2` |
| C3 | `workflow-block-2`, `workflow-missing-tool-2`, `workflow-timeout-4`, promotion after deleting external execution logs |
| D1–D3 | `lifecycle-*`, `workflow-rule-budget-2`, `cost-separation`, `skipped-activation`, `absent-oracle` |
| Parallel equivalence | `workflow-promote-1`, `workflow-promote-4`; `tests/selfcheck.rs` compares content digests and paired oracle outcomes |

`tests/selfcheck.rs` additionally substitutes the LF evaluator with a permissive
real evaluator and requires the unchanged protected oracle and promotion
goldens to reject the regression. Native workflows reject inherited Git
redirection before setup. These fixtures complement the integration targets
above; they do not replace the 18,000-file performance or live-tool gates.

Evidence retention and candidate revision commands are shown below. For
protected validation, signing contracts, promotion, active selection, rollback,
lifecycle and longitudinal reads, see [policy validation](policy-validation.md).
Evidence and candidate creation do not grant approval. After signed promotion,
ordinary checks use the immutable active package; local YAML cannot weaken it.

```bash
qualitygate policy evidence add --input evidence.json --source feedback.txt --format json
qualitygate policy evidence list --limit 32 --format json
qualitygate policy evidence show <evidence-digest> --format json
qualitygate policy candidate create --from-policy-ref HEAD --evidence <evidence-digest> --actor assistant --reason "Repeated false positive" --format json
qualitygate policy candidate rules enable <candidate-id> line-ending --actor assistant --format json
qualitygate policy candidate rules configure <candidate-id> test-naming --param 'patterns.rust="^test_"' --actor assistant --format json
qualitygate policy candidate rules disable <candidate-id> test-naming --actor assistant --format json
qualitygate policy candidate show <candidate-id> --format json
qualitygate policy candidate list --offset 0 --limit 32 --format json
qualitygate policy candidate reject <candidate-id> --actor reviewer --actor-kind human --reason "Insufficient evidence" --format json
qualitygate policy show <policy-digest> --format json
qualitygate policy history --limit 32 --format json
```

`evidence.json` is a strict JSON `EvidenceRecord`: `schema_version: 1`, `kind`
(`conversation_correction`, `check_report`, `review`, `failure`, `usage` or
`capability_gap`), `source_digest` (SHA-256 of the exact source bytes), `scope`,
nonzero Unix `timestamp`, `actor` (`id` and `kind: human|agent`), nonempty `claims`,
`sensitivity` (`public`, `internal` or `restricted`), `rule_ids`, `known_limits`
and `unverified_assumptions`. Input/source paths are confined repository-relative
files, each at most 1 MiB. Capability gaps describe missing capability and must
leave `rule_ids` empty. A correction or report is retained as unverified
evidence, with its exact bytes, and is never implicitly transformed into a rule.

`--from-policy-ref` accepts a Git ref or a previously archived policy digest.
Git refs resolve once. The archive copies the policy configuration, available
project rules, their normative sources, declared verification assets and the
frozen rule catalog. Other source-tree blobs are borrowed during selection and
are not copied into each candidate. A package is limited to 4,096 files and
32 MiB; archive objects and the index are at most 8 MiB each. Single-writer
transactions have a cooperative 30-second deadline. The committed index bounds
referenced object growth to 100,000 objects and 1 GiB. Failed transactions can
leave unreachable objects; they do not publish a candidate or active version.
Physical disk use including those recovery objects is not a hard quota.

Every successful semantic edit creates a new revision and policy digest,
retains the immutable parent and original creating actor, and records the
editing actor, mutation and prior revision. No-op edits preserve the current
revision. Rejected revisions remain listed and cannot be edited. Queries
paginate at 1..256 records and history returns a `next_cursor` for older events.

Objects live in `.qualitygate/policy/objects`; only `.qualitygate/policy/HEAD`
is atomically replaced, after flushing new objects and checking the original
head. Reads verify content digests and record types. Concurrent writers fail
on `write.lock`; crashes retain that lock for explicit recovery after the
writer is confirmed stopped. This is cooperating-writer atomicity and tamper
detection, not an OS access-control boundary. Repository operators must retain
or back up this ignored archive if they need durable cross-checkout history.

## Ownership and acceptance boundaries

Pure serializable records and acceptance decisions belong to `domain`.
Configuration, evidence storage, schema checks, atomic candidate transactions
and immutable content-addressed history belong to `config`. `application`
captures immutable Git inputs once, shares them between paired runs, runs
independent cases with bounded concurrency and assembles rich evaluation
records. `adapters` owns external signature normalization/verification;
`runner` retains process-group termination and output/time limits. Blocking
reads, serialization and CPU work run outside asynchronous orchestration.

The mutable workspace configuration remains an explicitly untrusted draft.
Published policy content is stored by digest and must never be edited in
place. Candidate operations cannot mutate their frozen parent, selected
protected tasks, trusted keys, acceptance criteria or previous evidence.
Approval binds the exact candidate, evaluation, acceptance epoch and parent;
later mutations invalidate earlier validation and authorization.

Baseline and candidate use the same frozen evaluator within an epoch. A
replacement evaluator requires independent anchor calibration before results
can be compared across epochs. Comparisons retain incomplete execution and
regressions separately from findings; incomplete execution cannot authorize
promotion. External authorization is an operator-controlled boundary, not a
key or approval manufactured by the CLI.

The failure archive retains successful, blocked, incomplete, rejected,
abandoned and rolled-back experiences distinctly. Lifecycle retirement changes
future selection without deleting published definitions or replay evidence.
Maintenance metrics must describe observations; missing use or benefit
evidence is unknown, never inferred from a rule merely existing.

## Performance acceptance

No unbounded task fan-out or full repository copy per pending evaluation.
Snapshot acquisition reuses the existing byte/file/job/time limits. Paired
evaluation shares immutable captured inputs, creates isolated execution
directories only for scheduled work, and has a global concurrency limit.
Result ordering and decisions are deterministic regardless of completion order.
Timeout, cancellation, worker failure or source mismatch produces incomplete
evidence and leaves a queryable failed attempt. Benchmarks compare serial and
parallel runs on controlled inputs with identical resource budgets, report
observed timings and enforce generous wall-time regression thresholds; a
universal speedup ratio is not assumed.

The current archive regression feeds 18,001 borrowed file inputs representing
over 35 MiB. It requires publication within five seconds and less than 1 MiB of
archived objects; only the policy configuration is retained from those file
inputs. On the shared Linux development host on 2026-09-14 this operation took
83.38 ms. This measures policy selection and publication from captured bytes,
not Git acquisition or paired evaluation. Run
`cargo test --all-features --test policy_candidates archiving_policy -- --nocapture`.
The local raw observation is `target/issue4-performance.log`.

## Current implementation verification

The 2026-09-14 Linux development run passed `cargo fmt --all -- --check`,
`cargo check --all-targets --all-features`,
`cargo clippy --all-targets --all-features -- -D warnings`, and
`cargo test --all-targets --all-features`. The complete native run passed
**335 tests**, including **185 unit tests**, with nine existing external-tool
scenarios explicitly ignored. The architecture/documentation target passed;
`target/architecture/report.json` retains 136 source digests and 1,350 file/line
dependency references with no ownership violations.
`cargo llvm-cov --all-targets --all-features --fail-under-lines 90` reported
**95.44% Rust line coverage**. Logs are `target/fixtures-check.log`,
`target/fixtures-clippy.log`, `target/fixtures-native-tests.log`
and `target/fixtures-coverage.log`.

The final development executable's full selfcheck passed **334 named verified
shapes** (`target/fixtures-final-selfcheck.json`), including **85 added policy
fixtures and 335 additional golden assertions**. All 249 original inputs and
goldens are preserved. The selfcheck integration target passed nine tests,
including evaluator mutation and inherited Git redirection regressions.
Separate nightly checks passed
**19 pure-domain tests under Miri** and **185 native unit tests under ASan**
with leak detection, using Rust 1.100.0-nightly (4b6d04e70, 2026-09-13).
Logs are `target/fixtures-miri.log` and `target/fixtures-asan.log`.
The separately built ASan CLI also passed all **334 selfcheck fixtures** with
leak detection enabled and no sanitizer stderr diagnostics
(`target/fixtures-asan-selfcheck.json`, `target/fixtures-asan-selfcheck.stderr.log`).
CodeSpec/Knowledge maps validated, including the new policy-evolution source
route (`target/issue4-map-validation.json`).

One additional full selfcheck launched during concurrent compilation and other
gates reached the unchanged 60-second corpus budget after 264 fixtures. It is
retained as **incomplete**, separately from the final complete run, in
`target/fixtures-selfcheck-under-load.json`. Neither budget exhaustion nor
missing execution was reclassified as a passing fixture; deadlines and goldens
were not relaxed. The final result is agreement on the verified shapes, not
proof of production correctness or a guarantee under arbitrary host load.

These observations establish the tested shapes and stated boundaries. Synthetic
reports do not establish real producer behavior, future agent benefit or
production correctness. Explicitly ignored external-tool scenarios and native
non-Linux execution are not established by this local run. The performance
contract and separate observations are in [large repositories](large-repositories.md).
