# Custom rule protocol, version 1

Syntax applicability uses the same [capability descriptions as initialization](init.md#discovery-output). Unsupported capabilities remain incomplete even when no entity matches; Shell currently supplies comments, not project imports or test methods.

Rule definitions are loaded from the **selected policy snapshot**. Staged checks read staged definitions; commit comparisons read the chosen commit. `--policy-ref` selects definitions from the caller-supplied policy reference and reports changes to candidate rule files. The caller remains responsible for establishing that reference's trust.

## Packages and activation

Built-in YAML manifests live in the installed Skill under
`references/rules/`; the binary reads them at runtime through the Skill layout
or `QUALITYGATE_BUILTIN_RULES_DIR`. There is no compiled rule-manifest fallback:
missing, malformed, incomplete, or unsupported Skill assets make policy loading
fail. `core` contains line endings, commit subjects and diff size; `shared`
contains general test, comment, quality, security, and configurable source
architecture rules. Selecting `lang-java` exposes `junit-naming` and the Maven
`module-boundary` and `used-undeclared` rules; `lang-python` exposes
`pytest-naming`.

Package selection makes definitions available. A rule runs only when enabled in `rules` and selected by the profile. Detection never enables source declarations automatically. Parameterization, comment language and diff size default to warning severity; explicit policy settings take precedence.

```yaml
schema_version: 1
rulesets: [core, lang-python]
custom_rules: qualitygate/rules
rules:
  pytest-naming: {}
  descriptive-tests: {}
```

Project rules are discovered recursively from the normalized repository
subdirectory named by `custom_rules`; `qualitygate/rules` is the standard
project-local discovery and generation location. Explicit older configured
paths remain supported for compatibility. Project definitions come only from the selected policy
snapshot: staged checks use staged bytes and `--policy-ref` uses the
caller-selected commit. A configured directory must exist and contain YAML.
Definitions are limited to 256 files and 1 MiB combined. Duplicate project IDs,
invalid YAML, unknown fields and unsupported protocol versions invalidate the
configuration. Local discovery additionally limits directory traversal to 4,096
entries and rejects symlinks.

An explicitly loaded custom definition can replace a packaged rule with the same ID. `rules list`, check metadata and the policy digest identify the selected definition and its origin. Changes are subject to the same policy-reference comparison. Two custom definitions with the same ID are always invalid.

`qualitygate rules list --language java --source builtin --format json` retrieves
all built-ins for Java, including language-neutral rules and unselected packages.
Use `--source project` for project definitions or `--source all` (the default)
for both. Omit `--language` for every scope. Identifiers are lowercase, with no
implicit aliases (`typescript` includes the JavaScript adapter's forms).
Queries work before `init`, discover `qualitygate/rules` by default and never
enable rules. An explicit policy `custom_rules` path takes precedence; malformed
available policy/assets fail closed. Both origins remain visible for duplicate
built-in/project IDs; only the selected definition can have `enabled: true`.
JSON includes `definition`, `language`, `source`, `overrides_builtin`, effective
configuration and source review bindings. `rules enable <id>` atomically updates
the candidate configuration and profiles, using only policy-selected definitions.
`config --show` includes the effective defaults and active catalog. Inventory
is not the execution plan. These commands support JSON, table and Markdown.
`init --config <path>` respects that path and never overwrites existing configuration.

## Schema-guided generation and validation

The Skill ships [project-rule.schema.json](../skills/qualitygate-cli/references/schemas/project-rule.schema.json)
and a complete [authoring workflow](../skills/qualitygate-cli/references/rule-authoring.md).
The CLI compiles that same Draft 2020-12 schema with external HTTP/file resolution
disabled. Both authoring and selected-snapshot loading enforce it, followed by
the Rust DSL validator; schema errors report instance and schema JSON Pointers.
Unknown keys, duplicate YAML keys, custom tags and non-JSON YAML shapes are rejected.
Languages are unique lowercase identifiers, with an empty list meaning universal.

```bash
qualitygate rules schema
qualitygate rules source --document AGENTS.md --section "Test naming" --format json
qualitygate rules validate candidate.yaml --format json
qualitygate rules generate --input candidate.yaml --format json
qualitygate rules validate --format json
```

`schema` always exports JSON and needs no repository. `source` computes the exact
current section binding. Construct a complete YAML/JSON candidate from actual
normative prose and the schema; the CLI does not infer obligations from prose.
`generate` checks schema, DSL semantics and the current source digest, then
atomically creates `qualitygate/rules/<id>.yaml` without overwriting or enabling
it. It checks the serialized output again. Project rule adoption still requires
an explicit `custom_rules: qualitygate/rules` policy and normal source review.
Generation also rejects ID collisions under other filenames and package budget
overflow. Use a single authoring writer; path checks are not an OS sandbox
against concurrent filesystem changes. The result reports `policy_changed: false`,
not an assertion that no existing policy already selects that ID.

`validate [path]` accepts a confined file or recursive YAML directory, defaulting
to `qualitygate/rules`. It returns 0 for valid candidates, 1 for invalid schema,
DSL, duplicate IDs or stale/ambiguous source bindings, and 2 for inaccessible
inputs or exceeded budgets. Reports retain violations when execution is
incomplete. Validation is not execution evidence or approval. The authoring
budget is 1 MiB per source/input, 1 MiB combined rule bytes, 256 rule files and
4,096 directory entries; diagnostics are bounded. Existing policy snapshot
source-review and trust requirements are unchanged.

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

The source hash is SHA-256 over the exact UTF-8 section bytes, including its top-level ATX Markdown heading, line endings and subsections, ending before the next supported heading of the same or shallower level. Supply 64 lowercase hexadecimal digits after `sha256:`. The section must occur exactly once as a CommonMark heading outside code, HTML blocks and quoted/list containers. The selector uses the raw ATX title after the heading markers, including inline markup. Setext headings are outside this source-selector profile. Changes to that section require review and an updated mapping; unrelated sections do not invalidate it.

The start is CommonMark's ATX marker offset; up to three spaces preceding the
first heading marker are outside the bound section. Use `rules source` so
authoring and snapshot validation agree on these exact offsets.

Every executed custom rule requires a [source review](source-reviews.md) in `source_reviews.<rule-id>` of the policy. Built-in rules require the same record when the policy assigns them a `source`. Missing or stale records keep required checks incomplete, including an initial mapping. The record binds the complete definition, version, source hashes and effective settings; changing only `content_hash` cannot restore a passing check. This tightens the earlier hash-only configuration contract. `rules list` reports the expected binding digest but does not issue an approval.

`schema_version` identifies the DSL protocol and defaults to 1 for the original requirement examples. `version` is the positive revision number of this particular rule; increasing it does not require a new protocol. Both appear in definition evidence and affect the policy digest.

The definition supplies default `required` and `severity`. Explicit settings under `rules.<id>` override those defaults and participate in policy comparison. Custom assertions are declared in the definition; arbitrary `parameters` are rejected. A policy-level `source` adds a source constraint and does not remove the custom definition's own source constraint.

## Entities and assertions

| Entity | Required capability | Supported `change` | Name/text used by predicates |
|---|---|---|---|
| `test_method` | `test_methods` | `added`, `modified`, `renamed`, `any` | Framework name / exact AST byte span, including attached decorators or attributes |
| `comment` | `comments` | `added`, `any` | Parsed comment text |
| `import` | `imports` | `added`, `any` | Parsed import text; this does not resolve package ownership |
| `file` | `files` | `added`, `modified`, `renamed`, `any`, `all` | Repository-relative path / UTF-8 file contents |
| `commit` | `commits` | `added` | Commit subject / complete commit message |

Omitted `change` means `added`. `any` selects current entities in changed files within the requested scope, including surviving tests in those files. `modified` tests have changed bodies or annotations. Deleted entities are not current validation targets. Unsupported entity/change combinations are rejected.

Files also support `change: all` and `max_lines`, `max_total_words`
and `required_paths` assertions. See [file contracts](file-contracts.md) for
measurements, empty-scope behavior and resource bounds.

`then.min_count` sets a lower bound on actually triggered entities for every
supported entity/change combination. Retained marker obligations do not fill
that count. Zero is legal, and the minimum cannot exceed `max_count`. Complete
empty scans fail when the minimum requires a match; missing capabilities or
failed parsing remain incomplete. Without a minimum, empty matching preserves
its existing behavior.

`then.require_pattern` requires a Rust regex match within each triggered
entity's text. For example, a file rule can require a nonempty ownership line
with `require_pattern: '(?m)^Owns: +\S.*$'`. Use separate reviewed rules for
independent fields, and `min_count` to require a nonempty target inventory.
This is text validation, not Markdown contract parsing or proof of ownership.
Deleting the field and emptying the selected inventory must exercise separate
negative fixtures. The four `custom-contract-*` minimal selfcheck cases cover
required text, zero entities and incomplete syntax; `tests/custom_rules.rs`
covers staged repairs and retained marker obligations.

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

`dependency_resolution` and `then.require_dependency` consume verified Maven or Python facts through rule-level `depends_on`; see [Maven facts](projects.md) and [Python facts](python-projects.md) for ecosystem semantics and retained-test obligations. Missing project evidence remains incomplete. `external_provenance` and `ai_only` consume [signed external run records](provenance.md), configured with rule-level `provenance.evidence_file`. AI-only scope is determined from authenticated replay before applying change predicates; current and retained declarations still carry marker/dependency obligations. [Git trailer bindings](git-trailers.md) require `commits` capability and associate each test with its last entity-changing commit, including merge-parent evidence and historical dependency obligations. The engine does not downgrade bindings or infer dependencies from imports. Remaining capabilities are in the [requirement ledger](implementation.md).

Rule metadata records the definition, origin, version, adapter version, selected change mode and matched count. `policy.rules_digest` covers the resolved settings, complete catalog and engine version; snapshot evidence covers the actual rule-file bytes. Diagnostics include the configured fix and a recheck command preserving selection, baseline, profile, task and policy reference.
