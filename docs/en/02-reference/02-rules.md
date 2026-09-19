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

Schema-guided workflow:

1. Export the matching `rules schema`.
2. Record the normative source and digest.
3. Author a candidate with stable IDs and explicit languages/capabilities.
4. Run `rules validate candidate.yaml`.
5. Review generated rules and counterexamples independently.
6. Publish to the project rule directory, then explicitly select it in policy.

Unknown fields, duplicate identities, invalid regexes, missing capabilities,
malformed syntax, and exhausted parser budgets fail validation or produce an
incomplete check. Rule generation is never approval.

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

### Repository prose to executable rules

`rules source` extracts one unambiguous section from `AGENTS.md` or another
repository policy and returns the exact source binding. An LLM may translate
only explicit, representable obligations into the finite DSL. File inventory,
counts, names, bounded text, markers, imports, and supported dependency facts
have structured representations. Graph, type, data-flow, runtime, or human
judgment obligations must instead use the appropriate lint/project adapter,
command check, or manual decision; they must not be approximated by a regex.

The [Schema-guided workflow](../01-user-guide/06-schema-guided-repository-rules.md)
shows the complete extraction, validation, generation, and separate adoption
sequence. The authoritative packaged procedure is the Skill's
[rule-authoring reference](../../../skills/qualitygate-cli/references/rule-authoring.md).

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
