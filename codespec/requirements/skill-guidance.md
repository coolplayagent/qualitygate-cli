# Skill guidance and design review requirements

This supplement governs the packaged Skill's modes and principle coverage.
The [research record](../../knowledge/best-practices/engineering-standards/guides/skill-contracts.md)
distinguishes source principles, repository-specific interpretation and available
evidence. It complements [clean contract maintenance](clean-contracts.md).

| ID | Requirement | Acceptance evidence |
| --- | --- | --- |
| SKILL-GUIDANCE-01 | Route verification, runtime/selfcheck, read-only discovery, rule authoring and design review by the requested task. Keep the detailed repository verification prerequisite and acceptance contract in one reachable owner. Missing configuration must not trigger init for runtime-only or discovery requests, nor stop independently authorized implementation or repository tests. | Skill reference reachability and canonical workflow checks; real startup mode tests; independent forward tests for intended and adjacent requests. |
| SKILL-GUIDANCE-02 | Four contextual cards cover responsibility/dependencies (SRP/DIP/Clean Architecture), shared knowledge (DRY), behavioral substitution/consumer interfaces (LSP/ISP), and evolution/clear intent (OCP/simple design/YAGNI). Each supplies concrete evidence questions, counterevidence and insufficient-evidence handling. No universal design score, size threshold or mandatory abstraction is inferred from these principles. | Packaged design cards and mapped primary research; independent review/forward-test artifacts showing specific evidence and counterexamples. |
| SKILL-GUIDANCE-03 | Design obligation samples remain optional, unadopted and instruction-only. The matching CLI validates exact source bindings and schema; independent fixture adoption, role scope, relocation and stale inputs retain existing semantics. Projection does not certify design or change a check verdict. | `tests/review_pack.rs` consumes both packaged review sets; Skill package/release inventory checks. |
| SKILL-GUIDANCE-04 | Runtime startup needs no repository configuration. Preserve an explicit selected executable and asset override; invalid explicit selections fail rather than silently switching. Version/digest/capabilities identify a build but do not authenticate its provenance. Development-runtime installation requires existing authorization for that update. | Real CLI startup tests for bundled/PATH/explicit selection, override failures, runtime-only mode and OS selection; selfcheck workflow review. |
| SKILL-GUIDANCE-05 | Repository verification preserves selected root/config/snapshot/policy/task/budgets, the init absence-versus-error distinction and doctor preflight limits. Completion still requires final unfiltered full scope, no pending delivery checks, complete passing gate, exit 0 and separately required checks. Quick/path, preflight and prompt delivery cannot substitute for that evidence. | Canonical workflow assertions, `tests/init.rs`, `tests/feedback.rs` and final snapshot-bound verification. |
| SKILL-GUIDANCE-06 | Validation distinguishes deterministic package/CLI/projection checks from semantic review and agent behavior. Forward testing retains prompt, exercised Skill revision/runtime/model, action trace, raw artifacts and outcome; unexecuted or incomplete observations are reported as gaps. Preserve existing gates, independent oracles and quality thresholds. | Focused Rust tests plus separately recorded independent forward-test artifacts; required repository checks remain unchanged. |

Executable rules may enforce explicit supported imports, file/entity predicates,
inventory relationships or configured analyzer/contract-test outcomes. Their
claims remain limited to the captured facts and declared scope. They cannot
infer arbitrary dependency graphs, shared intent, behavioral subtyping or
readability from text matching. Unsupported principles remain review obligations
or explicit evidence gaps rather than silently successful synthetic checks.

These requirements do not authorize policy adoption, external publication,
installation changes or fabricated approval. Existing user authorization remains
valid for its scope; the Skill must not create repeated approval barriers for
work already authorized. See the bilingual [evidence index](../../docs/en/04-contributor-guide/03-requirements-to-test-evidence.md)
for the maintained requirement-to-test mapping.
