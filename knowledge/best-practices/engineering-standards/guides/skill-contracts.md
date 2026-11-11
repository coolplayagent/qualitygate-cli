# Design principles in Skill guidance

Reviewed 2026-11-11. This review concerns the packaged Skill's decision guidance
and rule coverage. It complements the [code contract audit](clean-contracts.md);
it does not introduce a semantic analyzer or a universal design score.

## Sources and interpretation

Martin's [Clean Architecture](https://blog.cleancoder.com/uncle-bob/2012/08/13/the-clean-architecture.html)
distinguishes policy from implementation details and source dependencies from
control flow. His [SRP discussion](https://blog.cleancoder.com/uncle-bob/2014/05/08/SingleReponsibilityPrinciple.html)
relates cohesion to the people and concerns that cause changes. Neither implies
a fixed number of layers or a function-count threshold.

[Hunt and Thomas's DRY discussion](https://media.pragprog.com/titles/tpp20/dry.pdf)
concerns repeated knowledge and authority, including representations outside
code. Its age-versus-quantity example shows why identical validation text can
express different policies. Preserving independent test expectations is our
application of that distinction; sharing production logic with an oracle can
hide the defect the test should detect.

[Liskov and Wing](https://www.cs.cmu.edu/~wing/publications/LiskovWing94.pdf)
ground substitutability in behavioral properties, beyond matching signatures.
Martin's [SOLID discussion](https://blog.cleancoder.com/uncle-bob/2020/10/18/Solid-Relevance.html)
also addresses consumer dependencies, stable abstractions and extension.
We apply these ideas to Rust functions, adapters, protocols and instruction
interfaces without requiring inheritance or a trait for every function.

Fowler's [discussion of Beck's simple-design rules](https://martinfowler.com/bliki/BeckDesignRules.html)
prioritizes working behavior and communicated intent before removing unnecessary
elements. His [YAGNI discussion](https://martinfowler.com/bliki/Yagni.html)
distinguishes speculative future capabilities from useful current refactoring.
These support restraint, not removal of required validation or tests.

OpenAI's [Skill guidance](https://developers.openai.com/blog/rethinking-skills-and-prompts-for-gpt-6-astra)
recommends precise discovery, progressive disclosure and boundaries that preserve
the requested work. Its [Skill evaluation guide](https://developers.openai.com/blog/eval-skills)
separates observable outcomes and actions from qualitative judgement, using
retained traces and artifacts. We apply these recommendations to mode routing
and independent forward testing; they do not replace this repository's gates or
require a particular model for every consumer.

## Principle coverage and remaining evidence boundaries

The original public review pack covers concurrency, lifetime, bounds,
cross-path consistency and test quality. Those remain useful, but consistency
alone does not establish responsibility, common intent or substitutability.
The [design review](../../../../skills/qualitygate-cli/references/design-review.md)
adds four focused cards:

| Principle | Original guidance gap | New card and evidence | Executable boundary |
| --- | --- | --- | --- |
| SRP, DIP, Clean Architecture | Cross-path review did not explicitly identify change authority, source dependency direction or framework types crossing policy boundaries. | `design-responsibility`: name the owner, concrete change scenario, source edges and crossing types; distinguish control flow from dependency direction. | Declared `import-boundary` checks added supported syntax imports; Java project rules use their configured dependency facts. Full architecture graphs need an appropriate project check. None establishes semantic ownership automatically. |
| DRY | Similarity and path consistency did not distinguish one repeated knowledge claim from coincidentally identical code. | `design-knowledge`: identify competing authoritative representations and a change requiring coordinated edits; preserve trust-boundary validation and independent oracles. | Explicit schema, inventory and bounded collection contracts can detect declared drift. Text similarity cannot establish shared intent or justify merging unrelated policies. |
| LSP, ISP | Existing test-quality review did not explicitly compare replacement implementations or capabilities forced on real consumers. | `design-behavior`: compare caller inputs, outputs, errors, effects, ordering and resource bounds; identify unused dependencies and fake implementations. | Contract tests and configured compatibility matrices provide evidence for tested behaviors. Names, signatures and file presence cannot prove substitutability or consumer needs. |
| OCP, clear intent, simple design, YAGNI | Marker and test rules did not examine whether an extension point serves demonstrated variation or obscures the current behavior. | `design-evolution`: identify the current variation, compare direct and extensible designs, inspect names/comments/error meaning and measure performance claims. | Regression tests and project-specific measured budgets support a decision. There is no universal abstraction, readability, complexity or SOLID threshold. |

Counterevidence is required review work: a cohesive module can contain several
functions; an adapter can own platform branches; independent policy validations
can look alike; protocol snapshots can intentionally repeat old contracts; an
exhaustive closed-enum match can be clearer than a plugin system. An explicitly
versioned breaking change is distinct from an undisclosed implementation
substitution. Necessary current safety controls are not speculative features.

File contracts measure only their declared files, text and inventory. V2/V3
operators compose bounded `name`, `text` and `path` facts and collections; they
do not add arbitrary call graphs, type relations, ownership or data flow.
File correspondence does not establish test effectiveness. Missing capabilities,
incomplete input and exhausted budgets must retain their existing outcomes.

## Instruction ownership and authorization

The previous entry point repeated initialization and completion guidance in
operational references. Its verification prerequisite blocked rule discovery
despite documented discovery without policy, and startup examples required
repository configuration even for runtime-only requests. Completion scopes and
MR preflight exceptions also had more than one instruction owner.

| Instruction owner | Maintained responsibility |
| --- | --- |
| `SKILL.md` | Select the requested mode and retain authority/evidence invariants. |
| `references/verification.md` | Selected-policy prerequisites, automatic ordinary init when required, doctor and final-check acceptance. |
| `references/operations.md` | Snapshot/policy selection, resource acquisition, exclusions and trust inputs. |
| `references/runtime-startup.md` | OS-specific executable selection, explicit override errors and runtime-only identity queries. |
| `references/runtime-compatibility.md` | Capability/history/schema compatibility and interpretation of doctor evidence. |
| `references/selfcheck.md` | Fixture regression and explicitly authorized development-runtime updates. |
| `references/design-review.md` | Contextual design review, counterevidence and the four principle cards. |

Initialization blocks the Qualitygate verification conclusion when its required
policy is unavailable. It does not block independently authorized implementation,
repository tests or discovery. Runtime resolution does not silently replace an
explicit executable or authorize an installation. Provenance, executable
identity, compatible assets and successful fixture execution are different facts.

The four sample Agent obligations are optional, unadopted and instruction-only.
Using the cards in a requested review does not adopt repository policy. Persisted
adoption retains the existing exact source binding, separate reviewer decision,
role selection and stale-input behavior. A projected instruction cannot change
the deterministic gate or certify the principles.

## Validation and research limits

The [requirements](../../../../codespec/requirements/skill-guidance.md) and
[test evidence](../../../../docs/en/04-contributor-guide/03-requirements-to-test-evidence.md)
separate package reachability, real CLI startup, source-bound projection and
behavioral forward testing. Tests of links or schemas cannot establish that an
agent chooses the right mode or makes a sound design judgement.

The [forward-test record](../evidence/skill-forward-tests.json) retains prompts,
Skill digests, fixture inputs, action traces, outputs and observed outcomes for
two independent agents. The design review identified domain I/O and a timeout
substitution defect while rejecting superficial DRY/OCP findings. Read-only rule
discovery succeeded in an unconfigured repository without initialization or
enabling rules. Both are single-run observations, not a reliability benchmark.

The exact inherited model build was not exposed. The review trace was recovered
from retained conversation actions rather than a host audit log; its limits are
recorded. Runtime/CLI evidence is separate from semantic judgement, and Linux
observations do not replace native Windows checks. Missing observations remain
unverified; these examples do not prove semantic correctness or host enforcement.
