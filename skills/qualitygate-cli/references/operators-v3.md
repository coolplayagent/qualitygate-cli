# Named collections and lexical lambdas

Use `rules.operators.v3` and `schema operator-rule-v3`. Install the matching
precompiled CLI; evaluating these YAML/JSON rules needs no Rust, Node.js or
Python installation. The embedded evaluator runs bounded declarative operations,
not JavaScript/Python callbacks. Custom programs remain command checks.
V1 and V2 retain their schemas and original evaluation path; `schema operator-rule`
still exports V2. `schema project-rule-family` includes all three versions.

## File correspondence

This example convention requires every selected added, modified or renamed
`src/**/*.rs` file to have a current `tests/**/*.rs` counterpart. Compute the
expected path by replacing every literal `src/` with `tests/`, then every literal
`.rs` with `_test.rs`; retain the rest of the path, including subdirectories.
Deleted production files impose no counterpart obligation. Existing unchanged
tests count. An empty selected production set passes. This proves file
correspondence only; use `test_effectiveness` separately for behavioral evidence.

## Complete example

[The complete candidate](examples/operators-v3.yaml) is source-bound to the
section above and is validated by the integration suite. It is not enabled by
default. Copy this convention into your own policy only when it is appropriate,
obtain your own source binding, validate, generate and review before adoption:

```bash
qualitygate schema operator-rule-v3
qualitygate rules source --document AGENTS.md --section "File correspondence" --format json
qualitygate rules validate candidate.yaml --format json
qualitygate rules generate --input candidate.yaml --format json
```

Replace the example's `source` with the exact returned object; generation does
not enable a rule or approve its interpretation. Literal replacement does not
normalize paths, implement globs or interpret regular expressions.

## Sources and types

V3 adds `sources`, an object with up to 16 names. Names and variables use 1–64
ASCII letters/digits/underscores and start with a letter. Source name `entities`
is reserved. Each source declares `when`, `requires_capabilities`, optional
`language` and `applies_to.paths`. Selectors use existing adapter semantics:
`change: all` supports files only. Provenance/marker scopes on named sources are
unsupported. Top-level marker/dependency checks retain their existing behavior.

`entities` means the top-level triggered set. `{op: source, name: tests}` reads a
named source with its own selector, independent of the top-level scope and CLI
feedback path. Sources never expand the captured snapshot or bypass exclusions.
A path-limited run still cannot establish full delivery readiness. Every declared
source must be acquired completely before expression execution, including unused
sources. Source declarations and results bind to the same policy and snapshot.

`all`, `any` and `filter` accept optional `as`; `map` requires it. `{op: var,
name: source}` reads a lexical variable. Inner predicates can read outer
variables; rebinding an active name is invalid. `field` accepts optional `of`
with an entity expression; without `of` it reads the current predicate entity.
Only `name`, `text` and `path` exist. Fields on scalar values are invalid.
Path availability is checked for its required source, including unused branches.

| Operation | Input and result |
| --- | --- |
| `map(input, as, body)` | Entity/scalar collection to homogeneous bool, i64 or string collection. |
| `distinct(input)` | Scalar collection with duplicates removed; first occurrence order retained. |
| `contains(input, value)` | Scalar collection membership with exact type and value equality. |
| `subset(left, right)` | Every left scalar occurs in the same-type right collection; duplicates do not affect membership. |
| `count(input)` | Number of entity/scalar entries, including duplicates. |
| `all/any/filter(input, as?, predicate)` | Quantification/filtering over entity or scalar collections. |
| `concat(args)` | Concatenate 1–64 strings. |
| `replace(value, from, to)` | Replace all non-overlapping literal occurrences; nonempty `from`, empty `to` permitted. |

Existing bool/integer/string literals, boolean operations, typed comparisons,
fixed scalar membership and bounded regex remain available. The top-level
result must be boolean. Types are checked in every branch even for empty input.
Collections cannot contain collections or arbitrary objects. Functions,
recursion, dynamic fields, I/O and additional semantic inference are unsupported.

## Outer-variable alternative

The example's `subset(map(...), map(...))` can also be expressed with `all`
production files and `any` test file, comparing the mapped outer path with the
inner path. This expression fragment demonstrates explicit outer binding:

```yaml
op: all
input: {op: entities}
as: source
predicate:
  op: any
  input: {op: source, name: tests}
  as: test
  predicate:
    op: compare
    relation: eq
    left:
      op: replace
      value:
        op: replace
        value: {op: field, name: path, of: {op: var, name: source}}
        from: 'src/'
        to: 'tests/'
      from: '.rs'
      to: '_test.rs'
    right: {op: field, name: path, of: {op: var, name: test}}
```

## Completeness, budgets and observations

Complete empty sets retain `all=true`, `any=false`, `count=0`; require a positive
count when emptiness is forbidden. Missing facts, source parsing errors and
exhausted budgets are incomplete, not empty or passing results. Fixed assertion
violations remain visible if expression execution cannot finish.

Existing expression limits remain: 256 nodes, depth 16, 100,000 operations,
32 MiB cumulative scans and a 30-second evaluation deadline. Aggregate sources
are limited to 50,000 entities and 32 MiB name/text/path bytes, with a separate
30-second acquisition deadline and existing adapter limits. Intermediate
collections have at most 50,000 items and a shared cumulative 32 MiB materialized
budget. Strings and collection entries are charged before allocation; borrowed
references may be charged conservatively. Boolean/set membership operations do
not short circuit away work; large nested scans may exhaust the shared budget.

`metadata.expression` includes source counts, operations, scans, materialized
bytes and at most 64 false observations. Each observation locates the current
entity and lexical entity bindings using source name, index, file and range.
Mapped scalars retain their source origin where available. File previews are
bounded with truncation flags; omitted observations are counted. False inner
predicates are observations, not independent violations. A false root yields an
aggregate finding so delivery-line filtering cannot hide a collection failure.
`metadata.expression_inputs` records source declarations and snapshot identity.
