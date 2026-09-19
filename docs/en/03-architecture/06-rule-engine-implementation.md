# Rule engine implementation

[简体中文](../../zh/03-architecture/06-rule-engine-implementation.md) · [Volume index](README.md) · [Logical view](01-logical-view.md)

A catalog rule has a public ID and an `implementation` ID. The public ID keeps
policy and diagnostics stable; the implementation ID selects one closed Rust
evaluator. Configuration validation rejects unknown parameters, wrong types,
unsupported languages, bad globs or regexes, and unsafe combinations before
execution. The adapter creates a pending result, validates required facts,
dispatches the evaluator, and marks it complete or blocked. An unknown
implementation or missing capability can never become an empty pass.

The dispatch is in
[`adapters/rules.rs`](../../../src/qualitygate/adapters/rules.rs). Parameter
contracts are in
[`config/parameters.rs`](../../../src/qualitygate/config/parameters.rs) and
[`config/builtin_validation.rs`](../../../src/qualitygate/config/builtin_validation.rs).

## Progressive precision

Qualitygate uses the least expensive evidence level that can honestly support a
rule, while exposing the level and its limits:

```text
L0  snapshot and text signals
    line endings, commit metadata, added-file/added-line patterns
                         ↓ stronger evidence when required
L1  tree-sitter AST and canonical entities
    tests, annotations, imports, comments, ranges, base/head identity
                         ↓ ecosystem facts when syntax is insufficient
L2  project, compiler, and native-tool semantics
    modules, resolved dependencies, bytecode usage, lint/report evidence
```

There is no silent fallback from L2 to L1 or from L1 to L0. If an AST cannot be
parsed or a required project/lint producer is absent, the check is incomplete.
This is the main accuracy contract: fast text checks remain clearly labelled
review signals, while rules that claim structure use parsed nodes and rules that
claim dependency semantics require ecosystem evidence.

Tree-sitter currently provides a common structural base for Java, Python,
TypeScript, Go, Rust, and Bash/Shell. Adapters turn language-specific trees into
canonical tests, annotations, comments, imports, symbols, and exact ranges.
Base/head entity matching then distinguishes additions, modifications, moves,
and ambiguous copies. This foundation allows more text rules to migrate toward
AST predicates without changing the gate/report protocol. Future accuracy work
should add reviewed queries and typed capabilities, not disguise regexes as AST
or promise type/data-flow facts that tree-sitter alone cannot provide.

## Implementation families

| Family | Implementations | Input and purpose |
| --- | --- | --- |
| Snapshot metadata | `line-ending`, `commit-message`, `diff-size` | Captured bytes, selected commit subjects, and added-line totals. |
| Added-file text | `file-pattern`, `shell-shebang`, `shell-commented-code` | Whole newly added files or Shell-specific line groups. `file-pattern` uses bounded parallel batches. |
| Added-line text | `source-pattern` | Reviewed regex signals only on diff-added source lines. |
| Parsed entities | `test-naming`, `test-naming-strict`, `parameterized-tests`, `comment-language`, `ai-code-traceability`, `import-boundary` | Tree-sitter entities and their base/head identities. |
| Project semantics | `test-annotation-dependency`, `used-undeclared`, `module-boundary` | Snapshot-bound Maven/project facts and compiled usage. |
| Project DSL | custom-rule protocol v1 | A finite, schema-validated set of entity, text, marker, file-inventory, and dependency assertions. |
| Native tools | command checks and report adapters | Coverage, SARIF, tests, compatibility, and lint evidence from existing tools. |

Every family emits the same domain diagnostic: public rule ID, stable
fingerprint, optional file/range, message, structured evidence, repair guidance,
and recheck context. The fingerprint hashes the rule, file, and a
family-specific identity, so wording changes do not make one issue unrelated.

## Snapshot-metadata implementations

### `line-ending`

The evaluator visits every included changed file in the captured head. Binary
files containing NUL bytes are outside its text contract. Existing and renamed
files derive their expected style from the corresponding base bytes; new files
default to LF. Bytes are classified as LF, CRLF, mixed, or no newline, and only
a transition between nonempty styles is reported. The rule does not rewrite
files or consult mutable Git settings.

### `commit-message`

One validated regex is applied to the first line of every commit selected by the
snapshot comparison. Diagnostics are identified by commit OID and retain the
observed subject and pattern. A comparison with no new commits is explicitly
skipped, rather than presented as a checked empty inventory. The rule validates
subject shape, not authorship or signature.

### `diff-size`

The evaluator sums `Change.added_lines` across included paths and compares that
single total with a positive `max_added_lines` threshold. It produces one
aggregate finding. Renames without additions do not inflate the count, while
deleted lines do not offset additions. The metric bounds review size; it does
not measure semantic complexity.

## Added-file and Shell implementations

### `file-pattern`

This implementation selects only files whose change kind is `added`, applies
language routing and path globs, then scans every line in each selected file.
Explicit extension maps cover C/C++ in addition to registered languages. Work is
distributed through bounded parallel batches; a missing file or invalid UTF-8
blocks the result, and diagnostics are capped at 10,000. It is suitable for
“do not introduce this text in a new file,” not modified-line policy.

### `shell-shebang`

Changed files are routed as Shell by extension or recognized interpreter
directive. A missing `#!` is reported only when the file is new or line 1 is
added. That incremental condition avoids blaming an untouched historical first
line when another part of a script changes. Presence is checked; interpreter
availability and security are not.

### `shell-commented-code`

The evaluator groups consecutive comments that lexically resemble assignments
or Shell control/command keywords. A block of at least three lines is reported
when at least one line in the block is newly added. Block state is why this is a
separate implementation instead of a single-line pattern. It remains a lexical
heuristic: examples can match and other dead code shapes can evade it.

## Added-line implementation

### `source-pattern`

`source-pattern` is for inexpensive, auditable signals such as a direct API
call, suspicious option, or forbidden marker. It intentionally does not claim
AST, type, control-flow, or taint semantics.

1. `prohibited_patterns` is required and keyed by `all` or a supported
   language. It allows 1–32 language entries and 1–32 distinct regexes per
   entry; each regex is 1–512 bytes and is compiled during validation. Optional
   `paths` are repository-relative globs and `languages` narrows routing.
   Language-prefixed built-ins have fixed scopes; C-family rules explicitly use
   text-only C/C++ routing.
2. Runtime walks the snapshot's ordered change map, applies snapshot and rule
   path filters, ignores deletions, and requires every other selected changed
   file to exist in the captured snapshot.
3. It detects text-source language from the path and supported shebangs. Bytes
   must be UTF-8. Unsupported files are ignored; invalid selected text blocks
   the check instead of producing an empty scan.
4. It combines `all` and language-specific regexes, removes duplicate pattern
   strings, reads lines deterministically, and evaluates only line numbers in
   `Change.added_lines`. Retained lines and deleted code cannot create findings.
5. A match records language, exact pattern, and a one-line range. Its identity
   derives from path, line, and pattern. Multiple patterns can intentionally
   produce separate findings on one line.
6. One evaluation has a 30-second deadline, a one-million-added-line scope cap,
   and a 10,000-diagnostic cap. Exceeding a bound blocks the result as
   incomplete; output is never truncated into a pass.

Comments, literals, declarations, and unreachable code may match, while
multi-line or indirect behavior may not. Therefore `source-pattern` findings
are review signals, not proof of a defect or vulnerability. When false-positive
cost matters, move the rule to an AST entity/query or reuse a semantic lint.

## AST entity implementations

Entity collection parses both base and head for the complete change before path
filtering. It matches same-path symbols, then named moved bodies, then body
digests. The multiset algorithm prevents a rename from looking new and marks
ambiguous duplicates. Parsing shares a deadline and caps the collection at
50,000 test entities. Import rules inspect parsed import ranges intersecting
added lines; comment and test rules operate on typed ranges rather than raw
file text.

### `test-naming`

This implementation uses framework-aware parsed test entities and evaluates
only entities classified as added after base/head matching. It chooses a
configured per-language regex or a language default, then binds the diagnostic
to the entity's declaration range. It validates names, not test effectiveness
or discovery at runtime.

### `test-naming-strict`

The strict implementation shares the entity and matching pipeline with
`test-naming`, but configuration validation fixes its syntax scope to Java and
the catalog supplies the stricter naming contract. Keeping a distinct
implementation ID makes that fixed scope reviewable and prevents policy from
silently widening it through a language override.

### `parameterized-tests`

Added tests are grouped by enclosing symbol and normalized AST shape digest.
Entities carrying a recognized parameterization annotation are excluded. A
group reaching `minimum_similar` (default three, minimum two) creates one
heuristic finding listing its tests. Equal shape does not prove equal behavior,
so this rule suggests review rather than requiring an automatic rewrite.

### `comment-language`

Parsed comment nodes are selected when their ranges intersect added lines, then
reviewed exemption regexes are applied. English mode rejects CJK characters;
Chinese mode flags comments with no CJK character and at least three ASCII
alphabetic words; bilingual mode accepts both. This is a disclosed heuristic,
not a natural-language classifier.

### `ai-code-traceability`

Added tests, plus retained tests that previously carried an obligation, are
bound to an annotation, adjacent parsed comment, or verified Git trailer.
Required fields are parsed outside quoted prose so an example assignment cannot
satisfy a declaration. `ai_only` additionally requires verified external Agent
provenance; trailer mode requires commit-to-entity association. The result proves
the declaration binding, not the truth of the declaration or code correctness.

### `import-boundary`

Configured regexes run against parsed import-node text only when the node range
intersects an added line. AST ranges avoid matches in comments and ordinary
strings and preserve multi-line imports. The rule enforces a reviewed source
boundary; it does not claim resolved dependency or runtime data-flow semantics.

## Project-semantics implementations

### `test-annotation-dependency`

The AST stage selects added Java test methods with one configured annotation.
Only when such a test exists does the rule require Maven facts. Each selected
file must have exactly one owning module tied to the snapshot, and that module
must declare and resolve the configured test dependency. Imports alone never
stand in for module evidence.

### `used-undeclared`

Every configured module requires one Maven producer and a completed bytecode
usage inventory for the same snapshot. Each compiled use absent from direct
declarations retains scope, artifact type, classifier, analyzer, and producer
evidence. The rule evaluates the full module, not only changed lines, and cannot
prove reflective or runtime-resource dependency use.

### `module-boundary`

The evaluator builds a complete module inventory, selects declared or resolved
edges, deduplicates repeated transitive paths, and tests each edge against
reviewed `from`, `to`, and optional scope globs. One relationship yields one
finding listing all matched forbidden directions. Duplicate identities, foreign
facts, unsupported schemas, or missing manifests block evaluation.

Project rules never infer dependency semantics from import spelling. Each
module needs exactly one explicit producer, a matching snapshot digest, and a
manifest inside the checked snapshot.

## Custom-rule implementation

Custom protocol v1 is interpreted data, not executable plugin code. It selects
commit, file, test-method, comment, or import subjects and applies a closed set
of name/text, count, marker, dependency, required-path, line, and word
assertions. Entity changes reuse built-in AST multiset matching. A file rule
with `change: all` includes unchanged files under a 30-second, 50,000-file, and
32-MiB selected-text budget. AI provenance, Git trailers, and dependency
assertions require their explicit producers; missing capability is incomplete,
not “no match.”

## External lint/report implementation

External lint is not another catalog implementation string. It is a bounded
command check plus a typed report adapter. The runner fixes argv, cwd,
environment policy, deadline, output cap, accepted statuses, and artifact paths.
The adapter validates producer completion and normalizes native rule IDs,
locations, counts, versions, and raw-report digests. Ratchet mode reruns the
same producer on base and head before comparing counts or stable identities.

## Reusing lint instead of wrapping it

Qualitygate is not intended to reimplement Clippy, ESLint, golangci-lint, Ruff,
Checkstyle, PMD, SpotBugs, GCC/Clang analyzers, or compatibility tools. When a
mature tool owns the semantics, a command check runs that tool inside both
immutable snapshots and a report adapter preserves its native rule IDs,
locations, versions, completion markers, and raw-report digest. Ratchet mode
compares baseline/current counts or stable identities without an editable debt
file.

The added value is orchestration that an individual lint normally does not own:
Git snapshot identity, policy/task composition, bounded execution, report
completeness, cross-tool gate semantics, reproducible evidence, and Agent
recheck feedback. A wrapper would be excessive if it merely renamed a lint
message or approximated an available semantic rule with regex. A built-in is
justified when it supplies snapshot/diff semantics, a small portable convention,
cross-language consistency, or a capability absent from configured tools.

This split keeps the architecture extensible: add a parser/adapter for an
existing lint format, add AST queries for structural precision, and reserve
project/compiler adapters for facts neither text nor AST can prove. Policy can
combine all three without collapsing their evidence strength.

## Verification evidence

Focused tests in
[`adapters/rules_tests.rs`](../../../src/qualitygate/adapters/rules_tests.rs)
prove that `source-pattern` reports added lines only and blocks invalid
configuration. Issue integration targets cover routing, fixed package scope,
repair, invalid UTF-8, missing files, budgets, and deterministic ordering.
Structure, project, custom-rule, and report tests separately cover moves,
ambiguity, snapshot mismatch, missing facts, timeout, and malformed tool output.
