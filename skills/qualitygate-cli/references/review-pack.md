# Public review knowledge pack

This version 1 pack turns recurring review concerns into bounded review tasks.
The five topic cards and workflow obligations are guidance for the consuming
Agent. They do not prove that code is free of races, lifetime errors, or other
semantic defects. Project-specific protocols, API names and version matrices
remain in the project's own contracts.

The [obligation example](examples/review-obligations.yaml) uses the existing
[Agent contract](agent-contracts.md) protocol. It is an optional, unadopted
configuration fragment, not an automatically enabled rule package. Its six
obligations apply to `implementer` and `reviewer`, with no task-ID restriction,
at `start`, `after_change`, and `before_delivery`. These events describe
obligations; the projection supplies all applicable adopted instructions up
front. There is no automatic diff, path, or AST routing.

## Use in a repository

1. Copy the selected card sections, including their headings, into the target
   repository, for example as `qualitygate/review/review-pack.md`. Keep this
   installation guide in the Skill and keep project-specific additions in
   separately reviewed sources. Include the document in the selected snapshot;
   an installed Skill file outside the repository cannot serve as its source.
2. Select the relevant example obligations and merge them into the existing
   configuration's `agent_obligations` list without replacing other policy.
   Narrow roles or task IDs if appropriate. Empty task lists match every task;
   task selectors are IDs, not filenames.
3. For each selected card, obtain the exact `source` object from the matching
   CLI and replace the example's complete `source` object. The example is bound
   to this repository's packaged document; relocation changes its path.

   ```bash
   qualitygate capabilities --format json
   qualitygate schema agent-obligation --format json
   qualitygate rules source --document qualitygate/review/review-pack.md --section "Concurrency and atomicity" --format json
   qualitygate config --show --format json
   ```

4. Check `agent.contracts.v1`, validate each obligation against the exported
   Schema, and inspect the effective configuration. An authorized reviewer
   reviews the instructions and records adoption using the returned
   `agent_bindings` digest, review identity, reference and rationale. Do not
   invent approval or reuse adoption after changing the source or instruction.
5. Obtain `qualitygate agent-contract --worktree --role reviewer --format json`
   using the same base, policy and task inputs as verification. Apply its
   `prompt` and read the selected source cards before reviewing. Optional
   unadopted obligations remain visible as gaps and do not enter the prompt;
   `ready: true` alone does not mean that any card was adopted or reviewed.

All six examples use `instruction`, so they neither execute checks nor block
the code gate. Keep formal checks and review outcomes separate. When warning
findings need external assessment, the existing [judgment workflow](decision-protocol.md)
can triage one completed warning at a time; it is not a general diff reviewer.
Reload the projection after resume, handoff, or relevant input changes. Follow
the [Agent contract workflow](agent-contracts.md) for adoption and trust details.

For each selected card, record the inspected snapshot and scope, card ID and
version, outcome, evidence locations and remaining gaps. Outcomes are
`finding`, `no_finding_in_reviewed_scope`, `not_applicable`, or
`insufficient_evidence`. The latter two require a reason. A finding needs a
concrete counterexample and file/line, symbol, or call-chain evidence; absence
of a finding is not a correctness proof. These are review-record conventions,
not new CLI result fields or gate verdicts.

## Concurrency and atomicity

Card `review-concurrency`, version 1.

**Applicability:** changes to shared mutable state, atomics, locks, queues,
worker pools, process boundaries, or asynchronous callbacks. A keyword match
can prioritize review; absence of a keyword cannot establish non-applicability.

**Questions and evidence:**

- Identify each state's owner and thread/process sharing model. Cite the
  reads, writes, synchronization operations and relevant call paths.
- For read-modify-write sequences, identify the required indivisible operation
  and linearization point. Check whether failed compare-and-swap operations
  are handled and whether retries preserve the intended state transition.
- Compare lock scope with all protected fields and accesses, including error,
  cancellation and callback paths. Distinguish process-local memory from
  explicitly shared memory or inter-process synchronization.

**Counterexamples to check:** immutable data, thread confinement, externally
serialized callers, a correctly used atomic read-modify-write operation, or a
documented CAS retry loop can invalidate an apparent race. Cite that evidence
before dismissing or reporting the concern.

**Insufficient evidence:** if scheduling, ownership, synchronization contracts
or relevant callers are unavailable, name the missing facts and request the
corresponding contract, test or analyzer evidence. Do not infer a race solely
from separate atomic operations or an ignored-result text match.

## Pointers and lifetime

Card `review-lifetime`, version 1.

**Applicability:** nullable results, pointer/reference storage, shallow copies,
configuration replacement, pools, deserialization, callback captures or
asynchronous work that may outlive its initiating scope.

**Questions and evidence:**

- Trace allocation or acquisition, ownership, copying, release and use. Cite
  the relevant symbols and the call paths that connect them.
- Check whether null checks dominate uses on all relevant paths, including
  error exits. Distinguish a checked pointer from another alias or reloaded value.
- For escaping pointers, compare the owner's lifetime with the latest possible
  use. Include configuration reload, pool reset, cancellation and delayed
  callback execution; identify the operation that guarantees ordering.

**Counterexamples to check:** an owning copy, shared ownership, a stable arena,
non-null API contract or enforced join/cancellation protocol may make the use
valid. A nearby null check or shallow copy alone does not establish safety.

**Insufficient evidence:** report unknown ownership, aliasing, lifetime or
callback ordering explicitly. Suggest the missing API contract, reproducer or
supported analyzer check; do not label a possible escape as proven use-after-free.

## Bounds and integer safety

Card `review-bounds`, version 1.

**Applicability:** externally supplied lengths or indices, allocations, fixed
copies, integer conversions, subtraction, size expressions and retry bounds.

**Questions and evidence:**

- Identify each value's source, signedness, width, units and accepted range.
  Cite the validation and its relationship to every subsequent use.
- Check arithmetic before allocation, subtraction or copying, including zero,
  minimum, maximum, one-past-limit and conversion boundaries. Ensure the
  validation dominates the operation and is not invalidated by a later update.
- Compare copy length with both source availability and destination capacity;
  distinguish pointer size, element count and byte count. Trace retry counters
  through the first attempt, last permitted attempt and exhaustion path.

**Counterexamples to check:** bounded container operations, checked arithmetic,
validated types and proven upstream limits can discharge a concern. Check
their actual contracts rather than assuming all callers apply the same bounds.

**Insufficient evidence:** retain unknown ranges or buffer extents as gaps.
Give an input and arithmetic trace for a concrete finding; do not claim an
overflow or out-of-bounds access from a suspicious expression alone.

## Cross-path consistency

Card `review-consistency`, version 1.

**Applicability:** behavior changes with multiple entrypoints, protocol or
checker versions, upstream/downstream peers, process models, or rollback paths.

**Questions and evidence:**

- List the affected entrypoints, versions and peers. Write down the shared
  behavior contract and separately documented exceptions before comparing code.
- Trace a representative input through each relevant path, including defaults,
  normalization, feature switches, errors, logs and rollback behavior. Cite the
  implementation and contract for each comparison.
- Check that tests assert the intended contract rather than preserving a
  mistaken implementation. Identify missing paths or version combinations and
  connect each material difference to a test or explicit reviewed exception.

**Counterexamples to check:** intentionally different protocol versions,
compatibility requirements and separately specified responsibilities can
justify asymmetry. Similar names or code do not establish equivalent semantics.

**Insufficient evidence:** if the shared contract, version matrix or a peer's
behavior is unavailable, identify the missing owner/evidence. Do not invent a
universal behavior or treat an uninspected path as consistent.

## Test quality

Card `review-test-quality`, version 1.

**Applicability:** new behavior, changed assertions or fixtures, disabled tests,
mock changes, test renames and changes to discovery or test configuration.

**Questions and evidence:**

- Map each new behavior and relevant boundary/error case to an assertion and
  test invocation. Identify what incorrect implementation the assertion would
  reject; use old-code or mutation evidence when available.
- Check that setup and mocks represent reachable product states or explicitly
  justified fault injection. Verify discovery/execution evidence separately
  from the presence of a test function or a naming-pattern match.
- For disabled tests, inspect the reason, tracking reference and named
  replacement coverage. Compare the replacement's assertions and execution
  scope with the behavior that lost coverage.
- Check whether the name communicates expected behavior and conditions using
  the project's framework and naming contract. A valid name does not establish
  assertion quality or actual execution.

**Counterexamples to check:** intentionally tested exceptional states, property
tests, integration coverage and documented platform exclusions may be valid.
Explain why the evidence covers the behavior rather than demanding a particular
test style or one new test file for every source file.

**Insufficient evidence:** distinguish unavailable execution evidence from a
known missing assertion. Do not count mock plausibility, coverage percentages,
test names or an unexecuted replacement as proof of behavior coverage.

## Workflow obligations

Card `review-workflow`, version 1.

**Applicability:** review and delivery of behavior changes, disabled tests,
temporary fixes and responses to review findings. These obligations request
review evidence; instruction delivery alone cannot establish compliance.

**Required review steps and evidence:**

- For a disabled test, record its reason, tracking reference and replacement
  coverage. Review whether replacement assertions preserve the lost behavior
  coverage; merely supplying three nonempty fields is insufficient.
- For a temporary fix, record the root-cause tracking reference, accountable
  owner or explicit expiry condition, and removal plan. Keep the mitigation's
  limits visible; do not require removal of an honest marker to silence a signal.
- Ground review findings in file/line, symbols or call-chain evidence. When
  rebutting a finding, provide contrary code, contract or execution evidence
  rather than a bare disagreement. Preserve unresolved uncertainty.
- Map a behavior change to the affected design contract, tests, peer semantics
  and log schema. Update each affected artifact or explain why it is unaffected;
  changing one unrelated document does not satisfy the obligation.
- List affected entrypoints, versions, process models, upstream/downstream
  peers and rollback paths. Identify the owner of any unavailable evidence.

**Counterexamples to check:** truly unaffected surfaces and reviewed, scoped
exceptions should be recorded with reasons. A text signal may come from a
quotation or an example and require no product change.

**Insufficient evidence:** preserve missing tracking, coverage, contract or
impact evidence as review gaps. Report the inspected scope and outcome without
changing deterministic gate results or manufacturing approval.
