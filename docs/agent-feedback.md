# Agent repair feedback

`check --feedback` renders a bounded JSON view of the same gate decision used
by ordinary reports. It does not call a model or select different checks.

```bash
qualitygate --root /work/project check --worktree --base BASE_COMMIT \
  --policy-ref POLICY_COMMIT --task tasks/repair.yaml --profile full --feedback
```

Use actual reviewed commits and a task contract. A caller-supplied reference
retains its existing trust label; resolving it does not establish approval.
The complete, unfiltered JSON report is persisted before feedback is rendered.
`--feedback` selects JSON even with `--format table` or `--format markdown`.
The default output budget is 32 KiB, including the terminal newline;
`--feedback-max-bytes` accepts 4096–262144 only with `--feedback`.

## Version 1 envelope

The view has `kind: agent_feedback` and its own `schema_version: 1`.
It is not a replacement schema for full reports.

| Field | Meaning |
|---|---|
| `run_id`, `snapshot`, `policy`, `task` | Run, code digest, selected policy/configuration/rule/task identities and original report references |
| `gate` | Original `decision` and `complete`; exit codes remain 0 pass, 1 violation, 2 incomplete |
| `full_report` | Absolute report path, SHA-256 digest and exact persisted byte length |
| `findings` | Diagnostics of completed, applicable checks with a valid verdict |
| `execution_gaps` | Incomplete checks, status/reason, unverified diagnostic count and artifact references |
| `blockers` | Original gate blockers; execution gaps and violations remain distinct |
| `pending_delivery_checks` | Required checks omitted by the selected scope/profile |
| `acceptance` | Task acceptance-to-check mappings explicitly present in the plan |
| `recheck`, `delivery_recheck` | Whole argv arrays, or explicit omission and a pointer to full context |
| `delivery_ready` | Complete full task pass with no pending checks; covers the declared contract only |

Each inventory has `total`, `filtered`, `omitted`, `items` and `report_pointer`.
`total = filtered + omitted + items.length`. Severity filtering affects findings
only; it never removes execution gaps, changes the gate or filters the stored
report. It does not change whether required checks are blocking.

Text previews are limited to 256 UTF-8 bytes with `{text, truncated}`. Identity
and evidence locators are kept whole or referenced. Repeated fingerprints are
not merged: each shown diagnostic retains its original array index and evidence
pointer. `acceptance_link_known: false` means no explicit association was found;
it does not infer acceptance from diagnostic wording.

Each category gets an independent budget share. Whole items can be omitted;
`truncated` and category counts expose that omission. An item points into the
full report using JSON Pointer syntax. Follow its original artifact paths and
verify digests before relying on their contents. `evidence_availability` says
these files were not rechecked for continued availability. Moving or deleting
files does not change the recorded gate or make old evidence current.

## Rechecking and failures

Full reports gain additive optional `context` fields: `report_path`, `recheck`
and `delivery_recheck`. Their schema version remains 1. Rechecks retain the
selected base, task, configuration, trust inputs, budgets and snapshot selector.
Caller-selected policy references are pinned to their resolved commits.
`--expect-base` asserts the captured full base identity before running checks;
replay commands include it. Selectors with an implicit base, such as staged or
merge-request checks, fail incomplete if that base moves. Choosing a new baseline
requires an explicit new invocation.
Delivery rechecks set `full` and clear a path filter; a path-only selector becomes
a worktree selector. The argv selects a new check, not permission to reuse an
old snapshot or an old pass. Recheck an actual merge before delivery.

When command arrays do not fit they are omitted whole, with `/context/...`
references; never execute a partial preview. An exceptional identity/locator
overflow produces an explicit presentation error and exit 2, with the full
report location in the error. The persisted gate is unchanged. Configuration
or snapshot errors before a report exists retain the original incomplete error
object; consumers must distinguish it from an `agent_feedback` envelope.

## Requirement-to-test evidence

| Requirement | Evidence |
|---|---|
| EVO-02-01 | `domain/feedback_tests.rs::findings_gaps_and_pending_delivery_are_independent_and_gate_is_unchanged`; `tests/feedback.rs::simultaneous_findings_and_timeout_remain_visible_with_a_bounded_response` |
| EVO-02-02/03 | Domain bounded-output and legacy-report tests; CLI test `compact_feedback_retains_full_unfiltered_evidence_and_pinned_delivery_commands` verifies exact bytes/digest and unavailable evidence |
| EVO-02-04 | `tests/feedback.rs::path_and_policy_changes_preserve_scope_and_cannot_remove_acceptance`; actual pinned replay after a policy branch moves; staged replay rejects a moved implicit base |
| EVO-03-04 | `tests/pilot_baseline.rs::separately_passing_branches_require_a_new_full_check_after_merge` executes Cargo assertions before and after a real Git merge |

See the [external loop](agent-loop.md) and [phase-B record](pilot-phase-b.md)
for actual use and measured limits. Bytes are not token counts.
