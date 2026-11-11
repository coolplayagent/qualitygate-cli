# Clean code and executable contracts

Reviewed 2026-11-11. These primary-source readings inform repository design;
they are not new mandatory vendor controls or evidence of semantic correctness.

## Reading the principles in context

Clean Architecture separates policy from implementation details and directs
dependencies toward policy. For this CLI, serializable decisions belong in
`domain`; argument parsing, filesystem access and process execution have other
owners. A directory named `domain` does not establish independence if its types
derive CLI framework traits. [Martin, Clean Architecture](https://blog.cleancoder.com/uncle-bob/2012/08/13/the-clean-architecture.html).

SRP separates reasons for change. A rule's diagnostic wording changes for a
different reason from regex matching or snapshot selection. Moving that wording
to the rule definition improves ownership; splitting every expression into a
function would not establish SRP. [Martin, Single Responsibility Principle](https://blog.cleancoder.com/uncle-bob/2014/05/08/SingleReponsibilityPrinciple.html).

DRY addresses repeated knowledge and intent, including contracts represented in
different forms. Validation and discovery should share the same parameter
constraints. Independent known-answer tests should still record expected bytes
and failures: deriving an oracle from the implementation can hide a defect.
The oracle guidance is our application of the principle, not a claim from the
source. [Thomas and Hunt, DRY excerpt](https://media.pragprog.com/titles/tpp20/dry.pdf).

SOLID is useful beyond class inheritance: preserve behavior when exchanging
implementations, keep consumer interfaces focused, and separate stable policy
from details. Our Rust application uses small pure contracts and typed boundary
adapters. It does not require a trait, factory or plugin for every function.
Adding a source-pattern rule's literal guidance should not require adding an
evaluator branch keyed by its ID. [Martin, SOLID Relevance](https://blog.cleancoder.com/uncle-bob/2020/10/18/Solid-Relevance.html).

Simple design prioritizes working behavior, clear intent, removal of duplicated
logic and fewer unnecessary elements. We therefore retain public compatibility
wrappers and independent fixtures while consolidating their shared invariant.
[Fowler, Beck Design Rules](https://martinfowler.com/bliki/BeckDesignRules.html).
Speculative caching or a general rule language needs a demonstrated use case
and measured benefit before it earns additional state and invalidation logic.
[Fowler, YAGNI](https://martinfowler.com/bliki/Yagni.html).

## Applying the review

| Question | Concrete repository decision | Evidence boundary |
| --- | --- | --- |
| Who owns the reason for change? | CLI spelling and possible-value help live in `interfaces`; domain enums retain their serialized values. | Argument and serialization tests, plus architecture checks; not a universal SRP score. |
| Is one contract implemented twice? | `domain::rule_parameters` supplies language scope and bounded regex-map constraints to config discovery and evaluation. | Reject unknown languages, duplicate/invalid regexes and byte-budget overflow at both entry points. |
| Can a definition change without evaluator surgery? | Optional builtin-owned literal `diagnostic` metadata supplies message/fix; severity and verdict still use policy and evidence. | Closed schema, text budgets, generic fallback, independent expected diagnostics. |
| Are immutable identities stable? | One canonical SHA-256 byte function and lowercase format predicate; serialization and streaming framing stay with their callers. | Known vectors, exact wire identity and malformed-input tests. |
| Are inventory facts copied? | Asset discovery and selfcheck use `config::RULESETS`; `EmbeddedSuite` names fixture paths and bytes. | Independent fixture goldens, inventory checks and same-checkout corpus comparison. |
| Does orchestration block on filesystem calls? | Root resolution runs through an application blocking task and the `paths` owner. | Missing-root and symlink tests, with source/line architecture evidence. |

## Rules and prompts have different evidence

Use prompt/review cards to ask about responsibility, cohesion, error handling,
ownership and unnecessary abstraction. Attach concrete file/line reasoning and
counterexamples. A model's assertion that code is SOLID is not an acceptance
result. The [public review pack](../../../../skills/qualitygate-cli/references/review-pack.md)
uses explicit adoption and source-bound obligations for that workflow.

Promote a narrow claim to a deterministic rule only when its inputs and failure
meaning are stable. Owner import edges, declared language sets, bounded pattern
maps, digest syntax and fixture inventories meet that condition here. Regex
matches remain review signals; they cannot prove concurrency, lifetime or
architectural correctness. Missing evidence must remain incomplete execution.

## Continuing the audit

Preserve independent evidence before changing a contract; compare discovery,
validation, execution and replay; run the architecture gate after import changes;
then verify the final snapshot. See the
[requirements](../../../../codespec/requirements/clean-contracts.md) and
[test evidence](../../../../docs/en/04-contributor-guide/03-requirements-to-test-evidence.md).

Two findings need separate work: release archive verification still has explicit
asset lists, and repeated parser work needs a benchmark before adding a cache.
Neither is evidence that current release artifacts or parsing results are wrong.
