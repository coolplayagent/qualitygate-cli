# Evaluating adoption in a real repository

[简体中文](../../zh/01-user-guide/05-real-repository-pilot.md) · [Volume index](README.md)

After completing a normal check and repair cycle, you may want to measure whether
Qualitygate improves your team's work. Start with the decision you need to make:
which defects it catches, how much noise it adds, and how it affects repair and review.
A passing gate alone does not answer those questions.

## Choose the evidence you need

| Goal | Evidence to collect | Where to continue |
| --- | --- | --- |
| Adopt checks in a repository | Reviewed policy, tool prerequisites and a full report | [Rule configuration](03-rule-configuration.md) |
| Verify an installation or regression | Fixture outcomes and independent goldens | [Selfcheck and fixtures](../04-contributor-guide/02-selfcheck-and-fixtures.md) |
| Measure benefit against existing tools | Comparable real tasks, complete attempt logs and independent review | [Pilot evaluation](../04-contributor-guide/04-pilot-evaluation.md) |

## Prepare a pilot

Agree on real tasks, reviewers and measurable outcomes before collecting results.
Keep failed attempts, timeouts and missing evidence in the record. Use the
versioned pilot protocol when you need sealed plans, external authorization and
independently accepted results; its matrix and signature requirements belong to
the [evaluation procedure](../04-contributor-guide/04-pilot-evaluation.md).

Routine repository checks do not require a pilot. Fixtures and historical replays
remain useful engineering evidence, but do not establish real-world benefit.

---

[Previous: Agent repair and acceptance](04-agent-repair-and-acceptance.md) · [Contents](../README.md) · [Next: CLI commands](../02-reference/01-cli-commands.md)
