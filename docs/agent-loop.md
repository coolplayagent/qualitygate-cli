# External Agent repair loop

The opt-in [Rust example](../examples/agent_loop.rs) calls the same Qualitygate
CLI used by humans and CI. It owns attempts and evidence; the CLI owns checks,
snapshots and acceptance. It never accepts a model's completion claim as a pass.

Build the development CLI and example, then use a caller-created isolated Git
checkout, a reviewed task/policy, a UTF-8 task prompt and an existing evidence
directory outside that checkout:

```bash
cargo build --locked --bin qualitygate --example agent_loop
./target/debug/examples/agent_loop \
  --root /work/isolated-case --qualitygate /work/qualitygate-cli/target/debug/qualitygate \
  --base BASE_COMMIT --policy-ref POLICY_COMMIT --task tasks/repair.yaml \
  --prompt-file /work/case/task.txt --agent-command /work/case/agent.json \
  --output-dir /work/evidence --max-attempts 3 --timeout-seconds 1800
```

`agent.json` is a JSON argv array, not a shell command. The external agent
receives the prompt plus feedback on stdin. Configure the agent's own sandbox
and explicit model there. The harness invokes trusted executable paths supplied
by the operator; it is not a sandbox for arbitrary programs. Existing CLI tool,
rule-asset and environment configuration still applies.

## State and budgets

Base and policy references are resolved before the experiment. Every check is
full worktree scope with the fixed task and selected policy. The harness verifies
report location, digest (compact mode), gate/exit agreement, base, policy and task
identity, profile and pending checks. Changed policy or incomplete checks stop
the loop. Changing HEAD is rejected. Acceptance additionally requires a changed
source snapshot, so a no-op cannot count as a completed refactor.

`--feedback-mode compact` is the default. `full` supplies the unfiltered JSON
report to the agent under otherwise equal conditions. It is a feedback-format
comparison, not the agreed business trial's existing-tools control workflow.
The harness alone cannot establish comparable human effort or causal savings.

Limits are 1–3 attempts and 1–1800 seconds for all checks and agent work in one
run. Two consecutive attempts without a strict decrease in the original set
of error diagnostic identities/blockers stop with `no_progress`; swapping one
problem for a new one does not reset that budget. Other terminations include
`accepted`, `attempt_budget`, `time_budget`, `agent_failed`, `incomplete_check`
and `execution_incomplete`. Preflight or evidence-storage failures report
`configuration_or_evidence_error`. Only accepted runs exit 0; others exit 2.
Each change capture has up to ten seconds of bounded Git cleanup after a model
stops, including timeout. This cleanup is recorded separately from agent time.

A fresh `agent-loop-*` evidence directory retains:

- The fixed prompt/argv, checkout/base/policy identities, start time and harness
  source digest, limits and explicit unknown model/cost fields.
- Initial full report and feedback, then each prompt, captured model stdout and
  stderr, optional reported Codex token usage and full external recheck.
- Binary tracked patches against the fixed base, and separate untracked files
  with paths, modes, lengths and digests; symlink targets are retained as data.
- Per-state manifests and `index.json`, including failed and incomplete attempts.

Command output is bounded to 16 MiB per stream. Configuration/prompt files are
16 KiB, source replacements 1 MiB, and untracked capture 1024 files/16 MiB total
with a 2 MiB per-file limit. Non-UTF-8 untracked names are an explicit gap.
Partial stdout lost to an operator interrupt is not reconstructed: the saved
running attempt and termination remain incomplete. Keep evidence paths durable;
the harness does not upload or automatically repair moved artifacts.

## Read-only source proposals

When an Agent's write sandbox is unavailable, explicitly select
`--replacement-file src/ratchet.rs`. The agent stays read-only and returns a final
Codex JSONL `item.completed` / `agent_message` whose text is exactly:

```json
{"path":"src/ratchet.rs","content":"the complete replacement source"}
```

This is a distinct adapter mode. It only replaces the caller-selected existing
regular file under `src/`, rejecting traversal, symlink components, unknown JSON
fields, another path, oversized content and source changed while the model ran.
The controller preserves file permissions and writes through a temporary file.
It captures the resulting patch and runs the unchanged full acceptance contract.
No model command receives broader filesystem permissions.

The live [repair test](../tests/pilot_repair_live.rs) records the exact command,
model and output schema used locally. On the tested Linux host, Codex 0.154.0
read-only probes require its explicit legacy Landlock backend; workspace-write
fails because that profile cannot be represented by this backend. This is a
host-specific compatibility result, not a portable sandbox recommendation.
The live command uses Terra/Luna with equal `medium` effort, isolated sessions,
no user rules/config, approval `never`, and the same three-attempt/30-minute cap.

## Verification

Ordinary `tests/agent_loop.rs` tests exercise successful full rechecks, no
progress, attempt exhaustion, failed agents, changed selected inputs and
untracked evidence. Example unit tests reject redirected/stale/oversized source
proposals. They use temporary repositories and fixed non-model producers.

The billable live test is separate and never runs in the stable gate:

```bash
cargo test --locked --test pilot_repair_live \
  codex_models_repair_and_refactor_real_module_with_full_external_rechecks \
  -- --ignored --nocapture
```

See the [phase-B evidence](pilot-phase-b.md) for retained results and gaps.
