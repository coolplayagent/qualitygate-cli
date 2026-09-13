# Built-in structure rules

Built-in definitions are Skill resources, not compiled Rust string constants.
The matching executable reads `references/rules/{core,shared,lang-java,lang-python}`
from its installed Skill (or the explicit `QUALITYGATE_BUILTIN_RULES_DIR`). A
missing or malformed asset package is an error; it cannot silently fall back to
another rule set. Repository-local declarative rules are independently loaded
from the `custom_rules` directory in the selected policy snapshot (typically
`qualitygate/rules/`); see [custom rules and packages](custom-rules.md).

The current syntax adapters parse Java/JUnit, Python/pytest, TypeScript/Jest or Vitest-style test calls, Go testing functions, Rust test attributes, and Shell comments. Parse failures are reported as incomplete checks. Source parsing has a two-second per-file deadline and 200,000-node traversal limit, in addition to the snapshot byte budgets.

`test-naming` compares base and current entities and checks added tests. Default patterns preserve each framework's discovery conventions. Configure `parameters.pattern` for one pattern or `parameters.patterns` for a map keyed by language. `parameters.paths` confines the rule to explicit path globs.

`parameterized-tests` suggests sharing a parameterized test when at least three new tests within a file and class have the same syntax shape. Existing recognized parameterized tests are excluded. Configure `parameters.minimum_similar` (at least two). Its default severity is warning; increase severity only after the team's false-positive evaluation supports enforcement.

`comment-language` requires `parameters.language` to be `chinese`, `english`, or `bilingual`. It checks changed parsed comments, with `parameters.exempt_patterns` for terminology and code fragments. Language classification is a documented heuristic and defaults to warning severity.

`ai-code-traceability` requires an explicit `parameters.marker` binding with `type`, `name`, and optional `fields`. It checks declaration presence and fields. The default scope is all added tests; existing declarations retain their obligations. `parameters.provenance_scope: ai_only` selects agent-participating additions using [signed external run records](provenance.md). Missing scope evidence remains incomplete. [Git trailer bindings](git-trailers.md) associate declarations with actual entity-changing commits; uncommitted edits cannot borrow old trailers.

`security-sensitive-api` examines bounded language-keyed regular expressions on
changed Java, Python, Rust, TypeScript, and Go source lines. Its packaged
patterns identify process execution, dynamic evaluation, pickle loading, and
Rust `unsafe` use as warning-level review signals. A match is not proof of a
vulnerability; repositories can narrow `parameters.prohibited_patterns`,
`paths`, and `languages` only through an explicit policy change.

`todo-marker` is a warning-level changed-line review signal for `TODO`,
`FIXME`, and `XXX` in supported source files. It requires a disposition rather
than assuming a marker is a defect or should be erased.

`import-boundary` is a Java/Python/Rust/TypeScript/Go architecture rule. It is
disabled by default and can be enabled only with explicit language-keyed
`parameters.forbidden_imports` regex arrays. It evaluates added parsed imports,
not resolved dependency ownership, runtime reachability, or a threat model. An
enabled rule without a configured boundary is a configuration error.

```yaml
schema_version: 1
rules:
  test-naming:
    required: true
    severity: warning
    parameters:
      patterns:
        java: '^should_[A-Za-z0-9_]+_when_[A-Za-z0-9_]+$'
        python: '^test_[a-z0-9_]+$'
  ai-code-traceability:
    required: true
    severity: error
    parameters:
      paths: ['**/src/test/**/*.java']
      provenance_scope: all_added_tests
      marker:
        type: annotation
        name: AIGenerated
        fields: [author, date, description]
```

Core rules are `line-ending`, `commit-message`, and `diff-size`. Commit patterns use `parameters.pattern`; diff size uses `parameters.max_added_lines`. Enabling a rule edits a candidate configuration. Rules are not automatically imposed on repositories merely because an ecosystem is detected.

Each packaged definition declares `standard_refs` and
`lifecycle_inputs`. The catalog resolves both against the reviewed
[external standards archive](../knowledge/best-practices/engineering-standards/README.md)
and rejects absent, unknown, mismatched, or non-enforced mappings. The
`controls` for each reference must be declared by that source in the registry;
they are normalized provenance labels, not vendor-verbatim policy or an
independent enforcement decision. The definitions are exposed by `rules list`
and rule metadata.

When a packaged rule declares concrete `language` metadata, its exact language
set must equal the union of its concrete lifecycle-input lanes; an `all` lane
cannot make a Java- or Python-specific rule appear portable. An empty language
set remains the explicit form for a genuinely generic rule, whose lifecycle
input declares the supported adapter lanes. The catalog regression
`builtins_reject_language_scopes_that_mismatch_lifecycle_inputs` covers this
boundary.

The [lifecycle matrix](../knowledge/best-practices/engineering-standards/lifecycle-rule-matrix.yaml)
labels an input as enforced, evidence-contract, or planned. An enforced input
has deterministic snapshot evidence. Design review, security analysis,
performance, and operations entries require configured evidence and remain
incomplete when selected evidence is absent; planned entries cannot affect a
verdict. The [critical adoption guide](../knowledge/best-practices/engineering-standards/guides/critical-adoption.md)
records language, severity, tooling, and performance conflicts before a rule is
promoted.

The embedded archive accepts only its versioned registry schema and complete,
HTTPS-linked source metadata. Its closed lifecycle taxonomy and the required
status/outcome pairs (`enforced` → violation or warning, `evidence-contract` →
incomplete, `planned` → advisory) prevent a source note from silently changing
how an input can affect a verdict. Source-local normalized control sets prevent
a packaged rule from attributing an unarchived control to a reviewed source.
Its declared organization, language, SDLC,
and concern coverage is also checked: every dimension must reach a direct
lifecycle input rather than remain an unused bibliography entry. A universal
language input cannot stand in for a declared language lane.

The same matrix keeps risk-based security planning, supply-chain posture, and
release provenance as evidence contracts. They are not hidden default rules:
an adopting policy must select their scope and retain the specified requirement,
snapshot, report, artifact, and exception evidence before a gate can use them.

The `lang-java` package also provides `module-boundary`, which consumes Maven project facts from declared prerequisites. It requires an explicit module inventory and forbidden dependency directions; see [project rules](projects.md). This is a project semantic rule, separate from file syntax checks.

Unknown or wrongly typed parameters are configuration errors. Structure rules support `parameters.paths` and `parameters.languages`; line-ending, commit-message and diff-size do not accept those filters. See [custom rules and packages](custom-rules.md) for versioned definitions, capability declarations and source hashes.

Assigning `source` to a built-in rule also requires a [bound source review](source-reviews.md). This review is separate from packaged `standard_refs` catalog validation: it records the team's mapping from its own normative section to the effective executable rule.
