# Real-repository pilot protocol

This protocol covers REQUIREMENTS §9.2 and the real-repository evidence required by §9.1.3 and the rollout milestones. Automated fixtures, self-hosted checks and CI results establish implementation behavior. Human-confirmed violations, false-positive rates and review savings require observations from an agreed pilot and are not inferred from those tests.

No pilot has yet been accepted for this revision. Repository selection, sample size, observation period and acceptance thresholds remain team decisions. Until those inputs and the resulting review records exist, the implementation ledger keeps pilot acceptance pending.

## Before collecting outcomes

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
