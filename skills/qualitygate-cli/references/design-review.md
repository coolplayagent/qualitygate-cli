# Design contract review

Use these version 1 cards for design review or refactoring that affects ownership,
dependencies, shared contracts or implementation substitution. They apply Clean
Code, Clean Architecture, DRY and SOLID through concrete change scenarios; they
do not prescribe a universal class hierarchy, function length or design score.
Select relevant cards instead of applying every principle to every edit.

## Choose evidence before a rule

| Concern | Deterministic evidence available | Context still required |
| --- | --- | --- |
| Responsibility and dependency direction: SRP, DIP, Clean Architecture | Declared supported-language `import-boundary`; project architecture checks or configured analyzer reports | Which policy owns a change, what a boundary means, and whether data or side effects cross it |
| Shared knowledge: DRY | Explicit file/entity contracts, bounded collection comparisons, schema/inventory checks | Whether two copies represent the same knowledge and must evolve together |
| Substitution and consumer needs: LSP, ISP | Contract tests and declared compatibility matrices through configured command checks | Preconditions, errors, effects and capabilities required by actual consumers |
| Evolution and clear intent: OCP, simple design, YAGNI | Required regression tests and measured project-specific budgets | Whether an extension point serves demonstrated variation and whether the design explains its behavior |

Inspect [built-in definitions](builtin-rules.md) and the matching CLI before
selecting a rule. Import patterns inspect supported syntax, not all transitive
dependency edges. [V1/V2/V3 project rules](rule-authoring.md) use finite facts;
they do not infer arbitrary call graphs, subtyping, ownership or data flow.
File correspondence does not prove test effectiveness; line counts do not prove
SRP. Never invent a built-in `solid`, clone detector or semantic operator.

## Use and record a review

The [candidate obligations](examples/design-review-obligations.yaml) use the
existing [Agent contract](agent-contracts.md) protocol: optional, unadopted,
instruction-only entries for implementers/reviewers. They neither run checks
nor change gate decisions. To persist selected cards in a project's policy,
follow the [source binding and adoption workflow](review-pack.md#use-in-a-repository)
with this document and the selected section names. Copy sources into the target
repository, obtain exact bindings with `rules source`, validate against exported
`schema agent-obligation`, and keep adoption separate. Do not merge a sample
over the user's policy or manufacture reviewer approval. Review requested by the
user can use these cards directly without adopting new policy.

For each applicable card record the snapshot, inspected scope, card ID/version,
file/line or symbol evidence, concrete change/failure scenario and checked
counterevidence. Use `finding`, `no_finding_in_reviewed_scope`, `not_applicable`
or `insufficient_evidence`; explain the latter two. Separate project contract
violations from optional design recommendations. These are review conventions,
not new CLI verdicts. Preserve unknowns and avoid a blanket "SOLID passed" claim.

For a proposed refactor, state the contract it preserves, the authority that owns
it, the consumers affected and the smallest useful migration. Validate with
independent behavior evidence; [verification](verification.md) still governs
code-task completion. Use the [public review pack](review-pack.md) for detailed
concurrency, lifetime, bounds, cross-path and test-quality investigation.

## Responsibility and dependency direction

Card `design-responsibility`, version 1.

**Applicability:** changes that mix policy with I/O/framework details, move module
ownership, add dependencies or make unrelated consumers change together.

**Questions and evidence:** identify the business or technical actor responsible
for each change and the policy it owns. Trace source dependencies and data crossing
the boundary, including error types, framework annotations and serialization
formats. Distinguish source dependency direction from runtime control flow. Show
where filesystem, network, process or UI details enter policy decisions and which
consumer would change if those details changed. Cite the repository's actual
ownership contract before proposing a new layer or abstraction.

**Counterevidence:** multiple functions can serve one reason for change; adapters
may legitimately contain platform branches. A pure function need not acquire a
trait to satisfy DIP. Clean Architecture does not require exactly four layers.
Keep a boundary that already isolates the change; do not split code merely to
meet an arbitrary size or type-count target.

**Evidence limit:** import checks can enforce declared edges, but an allowed import
does not prove correct responsibility. If ownership, external type meaning or
relevant consumers are unavailable, record `insufficient_evidence` and name the
missing contract instead of inventing a forbidden dependency.

## Shared knowledge and authority

Card `design-knowledge`, version 1.

**Applicability:** the same policy, schema, invariant, default or algorithm appears
in discovery, validation, execution, replay, documentation or multiple consumers.

**Questions and evidence:** identify the knowledge claim and its authoritative
owner. Demonstrate a realistic policy change that requires coordinated edits to
multiple independent representations. Compare error paths and compatibility
versions as well as successful examples. Propose one authoritative representation
with explicit consumers, or document why separate representations are required.
Keep trust-boundary validation and independent test expectations intact.

**Counterevidence:** identical age and quantity checks may express different
policies and evolve separately. Revalidation across a trust boundary may be
necessary. Protocol snapshots, known-answer vectors and golden expectations
must not be generated from the implementation under test merely to remove
duplication. Share a stable invariant rather than couple unrelated contracts.

**Evidence limit:** textual similarity is a lead, not proof of duplicated
knowledge. Explicit inventory/schema contracts can be checked deterministically;
they do not discover common intent. If the change history or authority is unknown,
record `insufficient_evidence` before recommending a shared abstraction.

## Behavioral substitution and consumer interfaces

Card `design-behavior`, version 1.

**Applicability:** alternate adapters, implementations or protocol versions;
public API changes; broad interfaces that force consumers to depend on unused
capabilities or handle unsupported operations.

**Questions and evidence:** choose real callers and compare accepted inputs,
outputs, error categories, side effects, ordering, resource ownership and bounds.
Check whether a replacement strengthens preconditions, weakens guarantees or
converts timeout/incomplete evidence into success. For each consumer, identify
the capabilities it actually needs and any unrelated dependencies or fake/no-op
implementations it must supply. Require representative contract tests for
substitutable behavior, including failure and resource-boundary cases.

**Counterevidence:** matching signatures do not establish LSP. An explicitly
versioned breaking contract is not an undisclosed substitution. A cohesive
interface used as a whole need not be split into one-method traits; local helper
functions need not implement an interface. Consumer-specific views or adapters
may isolate a dependency without redesigning every implementation.

**Evidence limit:** name/text rules cannot prove behavioral substitutability or
consumer needs. If callers, external implementations or failure semantics are
missing, identify the untested paths and use `insufficient_evidence`. Do not
declare compatibility from successful-path tests alone.

## Evolution and clear intent

Card `design-evolution`, version 1.

**Applicability:** adding a supported variant, repeated policy edits in generic
execution code, new extension mechanisms, or abstractions that obscure intent.

**Questions and evidence:** identify the demonstrated axis of change and the
current requirement it serves. Check whether policy data can change through its
declared owner without unrelated evaluator branches. Compare a direct solution
with the proposed extension point, including state, invalidation, error handling
and migration cost. Read names and comments against actual behavior: make units,
ownership, effects and failure meaning clear where misunderstanding changes a
decision. Use measurements when performance motivates added complexity.

**Counterevidence:** an exhaustive match over a deliberately closed enum may be
clearer than a plugin framework. OCP does not prohibit modifying existing code;
unknown future variants do not justify a factory or cache. YAGNI does not justify
removing current validation, resource budgets, tests or necessary refactoring.
Prefer the smallest design that preserves the required contract and clear intent.

**Evidence limit:** structural thresholds cannot prove readability or the value
of an abstraction. If the promised variation or performance need lacks evidence,
record that gap and prefer the supported current requirement; do not label a
working direct design defective solely because it is not extensible.

## Sources and interpretation

These operational cards are this project's application of the principles, not
quotations or universal vendor requirements. Foundations include Martin's Clean
Architecture, SRP and SOLID essays, Hunt and Thomas on DRY, and Liskov and Wing
on behavioral subtyping. Fowler's simple-design and YAGNI discussions inform
the restraint criteria. The packaged cards are self-contained; using them does
not require fetching an external tutorial.
