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

### Version 2 composition

The [bounded operator protocol](08-bounded-operators.md) adds a typed expression
to v2 project rules. `rules schema` continues to export v1; use `schema
operator-rule` for v2 and `schema project-rule-family` for a mixed package.
Validation, source binding and policy adoption remain separate for both versions.

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
