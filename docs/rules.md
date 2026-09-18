# Built-in structure rules

See [rule management](rule-management.md) for `rules categories`, category
filters and assignments, `describe`, `configure`, and `disable`. Category labels
are mutable discovery metadata and do not enable checks.

Go import facts represent each single/grouped import spec once. Patterns see
the `import ` prefix plus the optional alias and package path. This preserves
individual changed ranges and avoids duplicate or historical group findings;
the [selfcheck corpus](selfcheck.md) includes both forms.

Built-in definitions are Skill resources, not compiled Rust string constants.
The matching executable reads `references/rules/{core,shared,lang-java,lang-python}`
from its installed Skill (or the explicit `QUALITYGATE_BUILTIN_RULES_DIR`). A
missing or malformed asset package is an error; it cannot silently fall back to
another rule set. Repository-local declarative rules are independently loaded
from the `custom_rules` directory in the selected policy snapshot (typically
`qualitygate/rules/`); see [custom rules and packages](custom-rules.md).

The current syntax adapters parse Java/JUnit, Python/pytest, TypeScript/Jest or Vitest-style test calls, Go testing functions, Rust test attributes, and Shell comments. Parse failures are reported as incomplete checks. Source parsing has a two-second per-file deadline and 200,000-node traversal limit, in addition to the snapshot byte budgets.

`test-naming` compares base and current entities and checks added tests. Default patterns preserve each framework's discovery conventions. Configure `parameters.pattern` for one pattern or `parameters.patterns` for a map keyed by language. `parameters.paths` confines the rule to explicit path globs.

[Issue 9](https://github.com/coolplayagent/qualitygate-cli/issues/9) adds four opt-in built-ins. `commit-message-convention` uses a ticket-prefixed commit-subject regex; configure `parameters.pattern` for a team's prefix and subject format. `test-naming-strict` applies `^should_.+_when_.+` to added Java tests and accepts `pattern` and `paths`. `test-annotation-dependency` selects added Java tests bearing `parameters.annotation` (default `Test`) and requires the Maven `parameters.group` and `parameters.artifact` in the owning test module's declared and resolved test classpath. Configure a snapshot-bound Maven project-facts command under `checks` and name it in the rule's `depends_on`; missing, stale or ambiguous facts make the check incomplete when an annotated added test is selected. An empty annotation selection passes without Maven facts if no producer prerequisite is configured; configured prerequisites remain required. `no-hardcoded-secrets` applies a configurable single-line `parameters.pattern` to added Java files; a match is a bounded literal-assignment signal, not a complete secret scan. The three Java rules require `languages: [java]`; all four default to error and are disabled until selected by repository policy.

[Issue 10](https://github.com/coolplayagent/qualitygate-cli/issues/10) adds three opt-in error-level built-ins over added files. `no-printf-log` detects `printf` and `fprintf` patterns in C/C++ source, while `no-unsafe-string` detects `strcpy`, `sprintf`, `strcat` and `gets`. Both require `languages: [c, cpp]`; `.h` is included for both languages. `no-test-sleep` detects `sleep`, `usleep` and `nanosleep` patterns in default test directories and filenames across languages; its `paths` parameter is configurable and its `languages: []` scope is fixed. These are single-line lexical signals, so comments, strings and declarations can match. They inspect only added selected files and require UTF-8 input. Missing files, malformed text, invalid parameters or exceeded budgets are incomplete rather than a clean pass.

[Issue 11](https://github.com/coolplayagent/qualitygate-cli/issues/11) adds six opt-in built-ins. `no-bare-except`, `no-os-path` and warning-level `no-print` inspect added Python file lines; `no-emoji` inspects added UTF-8 files across languages for its configured Unicode ranges. The file rules require their fixed `languages` parameter and accept `pattern` and `paths` overrides. They are lexical checks that can match comments or strings. `no-emoji` does not inspect commit messages or recognize every emoji sequence. `commit-message-format` checks added commit subjects against a configurable Conventional Commits subset; it skips comparisons with no new commits. `python-test-naming` checks added `test`-prefixed Python functions against `^test_[a-z][a-z0-9_]*$`; it fixes its language to `[python]` and accepts `pattern` and `paths`. Use `paths` to limit the rule to pytest test files; syntax parsing alone does not prove pytest will collect a file. PEP 8 allows narrow bare `except:` uses, PEP 428 does not ban `os.path`, and the stricter Python test name and commit type patterns are repository conventions. Warning findings remain visible without blocking an otherwise complete gate.

[Issue 12](https://github.com/coolplayagent/qualitygate-cli/issues/12) adds eighteen opt-in Shell rules with fixed `languages: [shell]`. Sixteen use bounded regular expressions on changed lines in `.sh`, `.bash`, and extensionless files with a recognized `sh` or `bash` shebang; `prohibited_patterns.shell` and `paths` remain configurable. `shell-missing-shebang` inspects the first line of added scripts or changed first lines, while `shell-commented-dead-code` finds three consecutive code-like comments when the block includes an added line. Extensionless files without a shebang cannot be identified as Shell. The regex rules are lexical review signals: debug mode does not establish a secret leak, SQL interpolation does not establish untrusted input, algorithm tokens do not establish security use, and some valid Shell syntax or strings can match. `shell-password-echo` recognizes simple `read password` forms and excludes `read -s`; more complex flag combinations need a Shell analyzer. `shell-stream-merge-position` checks the narrower form of a merge followed by a potentially conflicting redirection. Selected malformed UTF-8 input and analysis budget exhaustion are incomplete.

The Java syntax collector parses changed files on up to four CPU workers and preserves ordered results before matching test identity. The added-file scanner uses the same bounded worker design; each invocation has a shared 30-second deadline across its workers. Annotation dependency analysis is capped at 10,000 selected tests and the scanner at 10,000 diagnostics. Worker failure, syntax failure, timeout or excess findings are incomplete execution. The [large-repository contract](large-repositories.md#issue-9-rule-analysis) records the regression fixture and its limits.

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

Core rules are `line-ending`, `commit-message`, `commit-message-convention`, and `diff-size`. Commit patterns use `parameters.pattern`; diff size uses `parameters.max_added_lines`. Enabling a rule edits a candidate configuration. Rules are not automatically imposed on repositories merely because an ecosystem is detected.

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
