# Agent repair and acceptance

[简体中文](../../zh/01-user-guide/04-agent-repair-and-acceptance.md) · [Volume index](README.md)

`check --feedback` projects a bounded repair envelope from the complete report.
It includes stable diagnostic IDs, locations, evidence references, limits,
recheck arguments, pending delivery checks, and the original report identity.
It does not hide incomplete execution or authorize policy edits.

An external harness should keep this loop finite:

1. Select and retain the initial snapshot, policy, task, and full report.
2. Give the Agent only the relevant feedback and permitted source scope.
3. Record each proposal and attempt, including failures and timeouts.
4. Recheck the resulting worktree with the same authority inputs.
5. Stop on success, exhausted attempts or time, repeated no progress, or a
   non-repairable prerequisite.
6. Run a final unfiltered full check against the actual combined snapshot.

The harness owns retry and elapsed-time budgets. Budget expiry before Agent
launch, Agent timeout, command failure, and missing evidence remain distinct
states. A repair must not disable a required rule, reduce severity, edit the
trusted policy, or invent a manual approval.

Task contracts declare observable acceptance items and verification methods.
They add to repository policy and cannot weaken it. Command acceptance must
prove that the intended test ran; zero selected tests are not a pass. Manual
acceptance requires a caller-controlled trust store and a signed external
record bound to the task, snapshot, policy, acceptance item, validity window,
and reviewer scope.

Signed Agent provenance can authenticate a declared transformation history,
but it does not prove semantic correctness. Git trailers can associate a
declaration with the actual test-changing commit; they are evidence bindings,
not substitutes for execution. The final gate remains the authoritative result
for the selected snapshot and configured checks.
