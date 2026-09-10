# Custom rule protocol, version 1

Syntax applicability uses the same [capability descriptions as initialization](init.md#discovery-output). Unsupported capabilities remain incomplete even when no entity matches; Shell currently supplies comments, not project imports or test methods.

Rule definitions are loaded from the **selected policy snapshot**. Staged checks read staged definitions; commit comparisons read the chosen commit. `--policy-ref` selects definitions from the caller-supplied policy reference and reports changes to candidate rule files. The caller remains responsible for establishing that reference's trust.

## Packages and activation

The binary embeds the YAML manifests under `qualitygate/rules/`. `core` contains line endings, commit subjects and diff size; `shared` contains test naming, parameterization suggestions, comment language and source declarations. These packages are always available. Selecting `lang-java` exposes `junit-naming` and the Maven `module-boundary` and `used-undeclared` rules; `lang-python` exposes `pytest-naming`.

Package selection makes definitions available. A rule runs only when enabled in `rules` and selected by the profile. Detection never enables source declarations automatically. Parameterization, comment language and diff size default to warning severity; explicit policy settings take precedence.

```yaml
schema_version: 1
rulesets: [core, lang-python]
custom_rules: team-rules
rules:
  pytest-naming: {}
  descriptive-tests: {}
```

`custom_rules` names one repository subdirectory; its `.yaml` and `.yml` files are discovered recursively. Definitions are limited to 256 files and 1 MiB combined. Missing or empty configured directories, duplicate custom IDs, invalid YAML, unknown fields and unsupported protocol versions invalidate the configuration. Local discovery additionally limits directory traversal to 4,096 entries and rejects symlinks.

An explicitly loaded custom definition can replace a packaged rule with the same ID. `rules list`, check metadata and the policy digest identify the selected definition and its origin. Changes are subject to the same policy-reference comparison. Two custom definitions with the same ID are always invalid.

`qualitygate rules list --format json` shows definitions, sources, capabilities and enabled settings. `rules enable <id>` atomically updates the candidate configuration and its profiles. `config --show` includes the effective defaults and catalog. These commands support JSON, table and Markdown output. `init --config <path>` respects that path and never overwrites existing configuration.

## Definition

```yaml
id: descriptive-tests
schema_version: 1
version: 1
source:
  document: AGENTS.md
  section: Test naming
  content_hash: 'sha256:<replace with the exact section digest>'
language: [java, python]
required: true
severity: error
applies_to:
  paths: ['**/*.java', '**/test_*.py']
requires_capabilities: [test_methods]
when:
  entity: test_method
  change: added
then:
  name_pattern: '^(should_.+_when_.+|test_.+_when_.+)$'
fix: Rename the test to describe its expected behavior and condition
```

The source hash is SHA-256 over the exact UTF-8 section bytes, including its ATX Markdown heading, line endings and subsections, ending before the next heading of the same or shallower level. Supply 64 lowercase hexadecimal digits after `sha256:`. The section must occur exactly once outside fenced code blocks. Changes to that section require review and an updated mapping; unrelated sections do not invalidate it.

`schema_version` identifies the DSL protocol and defaults to 1 for the original requirement examples. `version` is the positive revision number of this particular rule; increasing it does not require a new protocol. Both appear in definition evidence and affect the policy digest.

The definition supplies default `required` and `severity`. Explicit settings under `rules.<id>` override those defaults and participate in policy comparison. Custom assertions are declared in the definition; arbitrary `parameters` are rejected. A policy-level `source` adds a source constraint and does not remove the custom definition's own source constraint.

## Entities and assertions

| Entity | Required capability | Supported `change` | Name/text used by predicates |
|---|---|---|---|
| `test_method` | `test_methods` | `added`, `modified`, `renamed`, `any` | Framework name / exact AST byte span, including attached decorators or attributes |
| `comment` | `comments` | `added`, `any` | Parsed comment text |
| `import` | `imports` | `added`, `any` | Parsed import text; this does not resolve package ownership |
| `file` | `files` | `added`, `modified`, `renamed`, `any` | Repository-relative path / UTF-8 file contents |
| `commit` | `commits` | `added` | Commit subject / complete commit message |

Omitted `change` means `added`. `any` selects current entities in changed files within the requested scope, including surviving tests in those files. `modified` tests have changed bodies or annotations. Deleted entities are not current validation targets. Unsupported entity/change combinations are rejected.

`then.name_pattern` requires a regex match on the name. `then.forbid_pattern` rejects a regex match in the entity's text. `then.max_count` limits the total number of selected entities, including zero as a valid limit. Multiple assertions are combined, with separate stable diagnostic fingerprints per entity and violation type. A definition must contain at least one assertion.

Tests are matched as multisets: unchanged symbols are reserved first, then removed bodies are matched to moves or renames. Copies remaining after those matches are new entities. Java method signatures distinguish overloads. Line movement alone does not create a new entity. Comment/import additions compare text multiplicities within the corresponding file, so shifted historical text is not new. Cross-file comment/import movement is not inferred as semantic identity.

Structure collection is limited to 50,000 test entities and 30 seconds per rule, in addition to snapshot and per-file parser budgets. It runs on a blocking worker rather than the async command scheduler. Missing adapters, syntax errors, unsupported required capabilities or expired budgets produce incomplete checks; successfully parsed scopes with zero matching entities produce completed checks with `matched_entities: 0`.

## Source declarations and capability limits

`require_dependency` also supports Python tests using a [Python installation producer](python-projects.md). Omit `group` or use `pypi`; Java still requires a Maven group. Names, markers and extras follow Python semantics. Both ecosystems retain dependency obligations on unchanged marked tests and route facts by language plus test root.

`then.require_marker: true` requires `binding.marker` and an explicit `applies_to.provenance_scope`. Annotation bindings require `annotations`; comment bindings require `comments`, in addition to `test_methods`.

```yaml
applies_to:
  provenance_scope: all_added_tests
requires_capabilities: [test_methods, comments]
binding:
  marker:
    type: comment
    name: '@generated'
    fields: [author, date, description]
when: {entity: test_method, change: added}
then: {require_marker: true}
```

This fragment belongs in a complete definition with ID, version, source and fix. Comment bindings require an immediately preceding comment block separated only by whitespace; intervening executable code cannot transfer a declaration to a later test. The marker begins a comment line, rather than appearing somewhere in unrelated prose. Fields must be nonempty assignments outside other quoted field values; a description containing `author='someone'` cannot supply an author field. Declarations describe claimed origin and do not prove how code was generated.

`dependency_resolution` and `then.require_dependency` consume verified Maven or Python facts through rule-level `depends_on`; see [Maven facts](projects.md) and [Python facts](python-projects.md) for ecosystem semantics and retained-test obligations. Missing project evidence remains incomplete. `external_provenance`, `ai_only` and commit-to-entity trailer association remain incomplete. The engine does not downgrade bindings or infer dependencies from imports. Remaining capabilities are in the [requirement ledger](implementation.md).

Rule metadata records the definition, origin, version, adapter version, selected change mode and matched count. `policy.rules_digest` covers the resolved settings, complete catalog and engine version; snapshot evidence covers the actual rule-file bytes. Diagnostics include the configured fix and a recheck command preserving selection, baseline, profile, task and policy reference.
