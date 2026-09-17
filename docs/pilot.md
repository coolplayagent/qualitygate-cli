# Real-repository pilot protocol

This protocol covers REQUIREMENTS §9.2 and the real-repository evidence required by §9.1.3 and the rollout milestones. Automated fixtures, self-hosted checks and CI results establish implementation behavior. Human-confirmed violations, false-positive rates and review savings require observations from an agreed pilot and are not inferred from those tests.

No pilot has yet been accepted for this revision. On 2026-09-16 the user confirmed qualitygate-cli, eight tasks (four bug fixes and four refactors), seven days, and the existing per-run budgets and acceptance thresholds. The concrete task inventory, reviewer assignment, monetary cap and durable evidence location remain to be recorded. Pilot acceptance remains pending until the resulting observations and review records exist.

The [phase-A record](pilot-phase-a.md) contains the current Codex two-model
preparation, confirmed observation parameters and actual engineering evidence.
Connectivity probes and controlled template fixtures remain separate from the
real-task observations governed here.
[Phase C](pilot-phase-c.md) provides the strict observation manifest,
digest-bound `pilot summarize` command and case-provenance contract. Its
retrospective module replay remains engineering evidence rather than this trial.
[Phase D](pilot-phase-d.md) adds a pre-observation integrity seal and balanced
model/workflow enrollment checks. It detects later plan changes but does not
authenticate who sealed the plan or when.
[Phase E](pilot-phase-e.md) binds the sealed subject to an external human-owner
DSSE/Ed25519 signature. It authenticates the configured owner and current trust
status, while leaving trusted time and final reviewer acceptance external.
[Phase F](pilot-phase-f.md) binds the complete manifest, report references,
authenticated start and structured threshold assessment to an independent
reviewer's signed acceptance or rejection.
[Phase G](pilot-phase-g.md) adds a v2 structured full-pilot budget, including
failed attempts and priced human review time, as a tenth signed threshold.
[Phase H](pilot-phase-h.md) adds a v3 declared task mix and checks that eight
distinct inputs represent four bug fixes and four refactors without reused task
IDs or contract digests.
[Phase I](pilot-phase-i.md) adds one archived issue/commit source artifact per
independent input and rechecks its size, digest and confined path on each call.
[Phase J](pilot-phase-j.md) seals an alternating assignment order and audits
declared actual start positions against that plan.
[Phase K](pilot-phase-k.md) binds an initial full report per run and audits
attempt count, elapsed time and the declared no-progress stopping rule.

## Before collecting outcomes

Complete the manifest without observations, then run `qualitygate pilot seal
--input pilot-plan.json --format json` and retain its output in the declared
external archive. The command requires real inputs, declared expected issues,
two requested model groups, both workflows and the same cohort matrix for every
input. Only actual routed model identities and observations may be added later;
any other plan change invalidates the embedded digest. See
[phase D](pilot-phase-d.md) for the exact boundary.

For a new pilot, start from `templates/pilot/observation-v7.json` and declare
the eight independent tasks, 4/4 task mix, source artifacts, total monetary cap,
human hourly rate, alternating `run_order`, each assignment's initial full report,
and `no_progress_limit=2` before sealing. Record each actual start position as
`start_sequence` in the observation. For each observed run, archive a v7 model
capture with assignment, Agent/harness, requested model, capture time and either
the reported actual model or an explicit unknown reason. The CLI compares the
bounded archived JSON with the manifest. The v1–v6 templates
remain for reading already prepared records; see [phase G](pilot-phase-g.md),
[phase H](pilot-phase-h.md), [phase I](pilot-phase-i.md),
[phase J](pilot-phase-j.md), [phase K](pilot-phase-k.md) and
[phase L](pilot-phase-l.md).

Before observations, run `qualitygate pilot authorization-subject --input
observations.json --format json`, have the configured owner sign the exact
subject in the external trust system, and retain the DSSE envelope beside the
external archive. During aggregation, pass both `--trust-store` and
`--authorization`; an absent or invalid authorization keeps `protocol_ready`
false. See [phase E](pilot-phase-e.md) for key scope, validity and revocation rules.

After the observation window and all reports are complete, run `pilot
acceptance-subject` with the authenticated start inputs. Retain the nine-check
assessment and have the configured independent reviewer sign the exact subject.
Pass that envelope as `pilot summarize --acceptance`; see
[phase F](pilot-phase-f.md) for decision and exit-code semantics.

Record the repository URL or local source, immutable baseline commit, permitted task types, observation period, sample size, and the owner who selected them. Use isolated clones for historical replay. Do not modify the reference repository or developer Git configuration. Distinguish historical replay, deliberately introduced failures, and ordinary development tasks; report each separately.

For each enabled rule, retain its normative source and hash, requiredness, severity, scope and planned mode. Record the trusted policy commit, task-contract digest, verification-asset inventory and expected required checks. Teams choose blocking rules before observing outcomes. Source declarations, comment language and similarity suggestions need explicit team scope and remain subject to their documented reliability limits.

Specify numeric acceptance thresholds for confirmed violation detection, reviewed false positives, eligible repair success, review burden and execution cost. Also specify a minimum review fraction, required-check completion rate and treatment of unavailable tools. Record thresholds before evaluating results; an unreviewed diagnostic cannot count as accurate or as a false positive.

Retain the existing compiler/test/static-analysis baseline: actual commands, versions, exit statuses, reports, elapsed times and the snapshots they ran against. Identify which checks the repository already enforced. Keep task types, input snapshots and available tools comparable between groups. If the groups are not comparable, document the difference and do not attribute outcome changes to qualitygate.

## Collection and repair

1. Run `qualitygate init --format json` in an isolated clone to record detection, supported capabilities and gaps. Review the generated candidate, selected checks, normative mappings and verification assets before establishing the pilot's trusted policy.
2. Select a real task or historical issue using the pre-agreed sampling rule. Record the issue/commit evidence and expected task behavior. Establish the task contract in the external harness and retain its approved digest.
3. Run the existing baseline tools and qualitygate against the same immutable source comparison. For commits use `check --diff <base>..<head>`; for edits use a pinned `--base` with `--worktree`. Include the task and trusted policy reference. Retain the complete JSON and all digest-addressed evidence, including unsuccessful or incomplete runs.
4. Have the reviewer classify independent diagnostics against the source and normative requirement. Record a canonical issue identity so repeated runs, line moves and duplicated tool reports do not inflate the count. Separate existing-tool findings, qualitygate-specific rules and completeness/policy failures.
5. Give valid actionable diagnostics to the agent with the agreed repair budget. Retain the original diagnostic, edits, attempt count and each recheck. Finish on the actual repaired snapshot with `full`, all required checks and the original task contract. Record unresolved/incomplete cases as such.
6. Collect review comments, rework rounds and active human time using the same measurement rule for all task groups. Record quick/full durations, environment and cache conditions separately, together with omitted, skipped and incomplete checks.
7. Publish the sample inventory and aggregate calculations alongside reviewer decisions and immutable run references. An authorized reviewer records whether each pre-agreed threshold was met. Retain failed cases and protocol deviations.

The `<base>` and `<head>` arguments above denote the actual pinned commit IDs recorded for each case. The CLI's `--policy-ref` verifies equality with the supplied policy; the external harness must authenticate the reference and expected task. See [execution evidence](reports.md#execution-and-tool-evidence) and [manual acceptance](manual-acceptance.md) for the available trust boundary.

## Measurement record

| Measure | Numerator / denominator or recorded observation |
|---|---|
| Confirmed violations intercepted | Distinct human-confirmed issues detected before submission; separately label findings already produced by existing tools |
| False-positive rate | Reviewed diagnostics judged false / all reviewed diagnostics; also retain total and unreviewed counts |
| Repair success rate | Eligible valid cases repaired and fully reverified within budget / all eligible valid cases assigned to the agent |
| Review burden | Relevant comments, rework rounds and active human minutes per comparable task; report group sizes and exclusions |
| Check cost | Quick/full duration distributions, sample counts, machine/tool/cache conditions and failures |
| Required-check completion | Completed required checks / expected required checks, with applicability and incomplete reasons retained |

Use null, with a reason, when a denominator is zero or evidence is absent. Keep warnings and errors separate where requiredness differs. Do not turn timeouts, unexecuted tests, missing capability, policy drift or stale snapshots into passing samples. A gate can complete and fail because it found a violation; completion and pass rate are separate measures.

Each case record needs a stable case ID, task type, real/history/injected origin, base/head IDs, initial and final snapshot digests, policy/task digests, environment/tool identities, command/report paths and digests, diagnosis IDs, reviewer classifications and identity, repair attempts and budget, final gate and delivery scope, timings, and any exclusion reason. Private source, credentials and personal review records can stay in the team's evidence store; the aggregate must still reference verifiable records.

## Available implementation evidence

The Rust suites already provide controlled check/fix/recheck fixtures for Java, Python and generated Rust projects, and explicit failure/incompleteness cases. They do not supply human review observations or real-repository benefit measurements. See [Maven](projects.md), [Python](python-projects.md), [interface compatibility](compatibility.md), [initialization](init.md) and [the implementation ledger](implementation.md) for their boundaries and actual verification results. Rollout remains pending until this pilot's measured acceptance record exists.

[Phase B](pilot-phase-b.md) provides actual Codex source proposals and external
full rechecks on extracted project modules. These controlled cases remain
separate from the agreed production task inventory and seven-day observation.
