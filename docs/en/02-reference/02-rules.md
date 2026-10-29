# Built-in and custom rules

[简体中文](../../zh/02-reference/02-rules.md) · [Volume index](README.md)

## Rule sources and categories

Rules come from version-matched Skill assets, repository packages under
`qualitygate/rules`, or a legacy configured custom-rule directory. Categories
organize discovery; packages and individual rules still require explicit
selection. Configuration preserves source identity, parameters, severity,
language, required capabilities, and review metadata.

Built-ins include language-neutral commit and file checks plus opt-in Java,
Python, TypeScript/JavaScript, Go, Rust, Shell, C, and C++ rule packages.
Structure-aware rules use tree-sitter where supported. Many security, logging,
style, and API checks are intentionally line-pattern review signals. Their
finding text states that a match does not prove exploitability, data flow,
runtime behavior, or standards conformance.

`import-boundary` remains disabled until a repository supplies reviewed
`forbidden_imports`. `security-sensitive-api` and `todo-marker` are warnings by
default. Language-specific rules should be enabled only after reading their
exact definition and capability limits.

## Project rule protocol

Version 1 YAML packages declare identity, applicability, source review,
entities, assertions, messages, and resource bounds. Assertions can require or
forbid bounded text and can require matching entity sets to be nonempty. They
cannot call arbitrary code or silently broaden their declared capabilities.

The following fields describe the v1 contract. For the extraction, validation,
generation and separate adoption steps, follow the
[authoring tutorial](../01-user-guide/06-schema-guided-repository-rules.md).

| Contract field | Meaning |
| --- | --- |
| `schema_version`, `id`, `version` | Protocol and stable rule identity. |
| `source` | Repository-relative document, exact section, and digest returned by `rules source`. |
| `language`, `applies_to` | Explicit language and path/provenance scope. |
| `requires_capabilities` | Facts the evaluator must possess; absence becomes incomplete. |
| `when` | A closed subject/change selector for commits, files, tests, comments, or imports. |
| `then` | Supported name, text, marker, dependency, count, required-path, line, or word assertions. |
| `binding` | Typed annotation, comment, or Git-trailer association when traceability is required. |
| `fix` | Actionable remediation derived from the same source clause. |

### Structured Schema contract

The project-rule JSON Schema is the executable serialization contract shared by
the CLI, Skill, generation, validation, and immutable policy loading. Objects
are closed with `additionalProperties: false`; conditional branches connect
each entity to its required capability and restrict combinations such as
full-inventory file assertions, marker binding, and dependency evidence.

`rules schema` exports the runtime copy. The matching Skill carries the same
bytes at `references/schemas/project-rule.schema.json`. Parsed equality between
those two documents establishes protocol compatibility; it does not establish
source approval. The CLI adds semantic validation for Rust regex/glob syntax,
confined paths, unique identities, exact source-section hashes, capability
combinations, and finite budgets.

## Select the rule protocol

Qualitygate 1.0.0 keeps v1/v2 compatibility and publishes V3 named collections
and lexical lambdas. The CLI version and rule `schema_version` are independent.
The published 0.5.9 binary does not advertise `rules.operators.v3`; use 1.0.0
for V3 and inspect the executable's capabilities before authoring.

| Rule format | Export from the matching CLI | Intended use |
| --- | --- | --- |
| v1 | `rules schema --format json` | Fixed file, text, name, marker and dependency assertions. |
| v2 | `schema operator-rule` | Typed bounded expressions over the selected entity collection. |
| v3 | `schema operator-rule-v3` | Named fact collections, lexical lambda variables and scalar transformations. |
| Mixed package | `schema project-rule-family` | Validate packages containing v1, v2 and v3 rules. |

```bash
qualitygate --version
qualitygate capabilities --format json
qualitygate schema operator-rule-v3
qualitygate schema project-rule-family
```

Runtime capabilities, exported schemas and source binding must all match the
selected protocol. Keep supported v1/v2 rules in their existing format unless a
new requirement needs V3; a version bump alone is not a rule migration.

## V2: bounded expressions

Capability `rules.operators.v2` introduces project-rule `schema_version: 2` with
a required typed `expression`. Existing v1 schema and semantics remain unchanged.
Export `schema operator-rule`; default `rules schema` still describes v1.
The [packaged operator reference](../../../skills/qualitygate-cli/references/operators.md)
contains the full operation set, example, limits and authoring workflow.

### Expression semantics

Expressions compose existing text/AST entity facts with boolean logic,
`all/any/filter/count`, typed comparison, membership and regex. Fields are
lexically bound `name`, `text` and `path`; no arbitrary code or dynamic fields
run. Existing fixed assertions remain additive, including marker retention and
dependency checks. Expression collections contain triggered entities selected
by the rule, and do not expand the adapter's semantic knowledge.

Type/shape validation covers every branch. Fact collection must complete before
evaluation; declared dependency/provenance inputs must retain their binding.
Complete empty sets have defined semantics: all is true, any false and count
zero. Explicit counting prevents vacuous success when at least one entity is
required. Parse failures, missing facts and exhausted budgets stay incomplete.
Earlier fixed-assertion findings are retained even if the expression cannot finish.

### Evaluation bounds and reports

One evaluation budget covers nested operations: 256 nodes, depth 16, 50,000
entities, 32 MiB of facts, 100,000 operations, 32 MiB scanned strings and a
30-second deadline. Literal/regex compilation bounds are separate and strict.
Boolean shortcuts cannot hide an incomplete declared source or bypass type validation. Existing acquisition
budgets still apply; no additional general-purpose execution runtime is introduced.

The primary selection and all named sources share one acquisition deadline and
entity/byte allowance. Selectors reserve normalized facts before copying them;
syntax parsing and entity matching inherit the remaining time. Exhaustion leaves
the check incomplete and preserves fixed violations already established.

The report preserves the original snapshot, policy and source-review binding.
Successful evaluation includes operation/scan metrics; false expressions produce
aggregate violations. Mixed authoring packages identify the exported family
schema and each rule file's actual contract. Schema validity and local source
reviews remain distinct from independent policy approval and logical adequacy.


### Author and validate an expression

Confirm `rules.operators.v2` in `capabilities`; export `schema operator-rule`
for the complete v2 shape. This fragment requires at least one selected entity
and checks every selected name. Combine it with `when.entity: test_method`, the
`test_methods` capability, identity, source binding and the other required fields:

```yaml
expression:
  op: and
  args:
    - op: compare
      left: {op: count, input: {op: entities}}
      relation: ge
      right: {op: number, value: 1}
    - op: all
      input: {op: entities}
      predicate:
        op: matches
        value: {op: field, name: name}
        pattern: '^test_'
```

Save the complete rule as `candidate.yaml`, run `rules validate candidate.yaml`,
then `rules generate --input candidate.yaml` within the authorized authoring
workflow. V1 rejects `expression`; generation does not enable the rule.

## V3: named sources and lexical lambdas

`rules.operators.v3` adds an independent `schema_version: 3` contract, exported
by `schema operator-rule-v3`. V1/V2 behavior and `schema operator-rule` stay
unchanged. The family schema includes all three versions. Use the matching
precompiled qualitygate binary; no Rust, Node.js or Python runtime is needed.
Confirm `rules.operators.v3` in `qualitygate capabilities --format json` before
using V3 rules; a matching version number alone does not establish support.

Python and Node.js projects use the same YAML/JSON expressions. Select their
file extensions and correspondence conventions in `applies_to.paths`, source
selectors and string operations. Git remains required for repository snapshots.
Lambda bodies are declarative expressions, not JavaScript/Python callbacks;
configured command checks still need their declared tools and runtimes. The
Unix integration suite verifies validation and full rule execution with only
Git on `PATH`, including a missing counterpart and a stale rule-source binding.

Rules can declare multiple independently selected fact collections, bind
lexical variables, capture outer entities, and compose scalar `map`, `distinct`,
`contains`, `subset`, `concat` and literal `replace`. All declared sources must be
complete and snapshot-bound; absent facts cannot become empty collections.
Top-level triggering and fixed assertions retain their meaning. Named file
sources can explicitly include unchanged context without broadening the
snapshot or its exclusions. AST full-inventory selectors remain unsupported.

[The complete V3 reference and source/test example](../../../skills/qualitygate-cli/references/operators-v3.md)
define types, source selectors, scopes, allocation budgets and diagnostics.
Correspondence checks prove only file relationships; behavioral evidence still
requires configured tests and, when appropriate, `test_effectiveness`.

V3 `distinct` and `subset` use bounded scalar indexes to avoid repeated pairwise
comparisons. Collections with few distinct values retain a linear path; general
indexes use stable ordering and exact comparisons. Index storage, comparisons,
scanned strings and index moves consume the existing shared budgets. `distinct`
preserves first occurrence order and source origins. Few subset lookups scan
directly to avoid sorting a large right operand. Both set operands are fully
evaluated; an empty left subset needs no lookup index. Larger inputs can still
exhaust a budget and remain incomplete.

## Reports, file contracts, and triage

Analyzer ratchets are command checks whose reports are normalized on both
baseline and current immutable snapshots. File contracts apply to the complete
declared inventory, including unchanged owner/test files when configured.
Neither should be represented as a new built-in rule.

Rule triage separates proposals that can be represented by bounded textual or
structural evidence from those requiring types, data flow, control flow,
lifetime, dependency resolution, or human judgment. Unsupported semantic
proposals stay documented as gaps instead of being approximated by a misleading
regex. Normative source reviews map exact external sections to executable
behavior and retain the source digest, reviewer, date, and known divergence.

For the dispatch pipeline, lint reuse, diff/entity matching, and
implementation-specific algorithms, see
[Rule engine implementation](../03-architecture/06-rule-engine-implementation.md).

---

[Previous: CLI commands](01-cli-commands.md) · [Contents](../README.md) · [Next: Projects and tool reports](03-projects-and-tool-reports.md)
