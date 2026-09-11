# Built-in structure rules

The current syntax adapters parse Java/JUnit, Python/pytest, TypeScript/Jest or Vitest-style test calls, Go testing functions, Rust test attributes, and Shell comments. Parse failures are reported as incomplete checks. Source parsing has a two-second per-file deadline and 200,000-node traversal limit, in addition to the snapshot byte budgets.

`test-naming` compares base and current entities and checks added tests. Default patterns preserve each framework's discovery conventions. Configure `parameters.pattern` for one pattern or `parameters.patterns` for a map keyed by language. `parameters.paths` confines the rule to explicit path globs.

`parameterized-tests` suggests sharing a parameterized test when at least three new tests within a file and class have the same syntax shape. Existing recognized parameterized tests are excluded. Configure `parameters.minimum_similar` (at least two). Its default severity is warning; increase severity only after the team's false-positive evaluation supports enforcement.

`comment-language` requires `parameters.language` to be `chinese`, `english`, or `bilingual`. It checks changed parsed comments, with `parameters.exempt_patterns` for terminology and code fragments. Language classification is a documented heuristic and defaults to warning severity.

`ai-code-traceability` requires an explicit `parameters.marker` binding with `type`, `name`, and optional `fields`. It checks declaration presence and fields. The default scope is all added tests; existing declarations retain their obligations. `parameters.provenance_scope: ai_only` selects agent-participating additions using [signed external run records](provenance.md). Missing scope evidence remains incomplete. [Git trailer bindings](git-trailers.md) associate declarations with actual entity-changing commits; uncommitted edits cannot borrow old trailers.

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

The `lang-java` package also provides `module-boundary`, which consumes Maven project facts from declared prerequisites. It requires an explicit module inventory and forbidden dependency directions; see [project rules](projects.md). This is a project semantic rule, separate from file syntax checks.

Unknown or wrongly typed parameters are configuration errors. Structure rules support `parameters.paths` and `parameters.languages`; line-ending, commit-message and diff-size do not accept those filters. See [custom rules and packages](custom-rules.md) for versioned definitions, capability declarations and source hashes.
