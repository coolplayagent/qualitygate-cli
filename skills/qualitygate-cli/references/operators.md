# Typed bounded operators

Requires `rules.operators.v2`, available from v0.5.8. Compare capabilities and
executable digest, not only version.
Export `qualitygate schema operator-rule` and compare it with the packaged
[v2 schema](schemas/operator-rule.schema.json). Default `rules schema` and
`schema project-rule` still export strict v1; v1 rejects `expression`.

Use `schema_version: 2` with the usual identity/version, source from `rules source`,
language, capability declarations, `when`, scope and fix. A v2 rule requires
`expression`. Optional `then` retains the existing fixed assertions; these and
the expression must all pass. Validate and generate through the matching CLI.
Generation does not enable a rule or authorize source review/adoption.

This expression fragment requires at least one selected entity and checks all
selected names; use it with the AST `test_method` selector and its declared
`test_methods` capability:

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

The finite operations are `bool`, `number` (signed 64-bit integer), `string`,
`field`, `entities`, `and`, `or`, `not`, `all`, `any`, `filter`, `count`,
`compare`, `in`, and `matches`. Fields are only `name`, `text`, and `path`,
bound inside collection predicates. Nested predicates bind their own current
entity; there are no arbitrary names, outer-variable capture, callbacks or code.
`filter` returns an entity collection and `count` returns an integer. `compare`
uses `eq/ne` for same-type scalars and `lt/le/gt/ge` for integers. Membership
sets use tagged scalar entries such as `{type: string, value: tests.rs}`.
`matches` uses bounded Rust regex. Unknown operators/fields and type mismatches
are invalid even in unused branches or empty collections.

`entities` contains the complete triggered collection produced by existing
`when`/`applies_to` selection. Retained marker entities continue through fixed
marker assertions; they are not added to the expression's changed collection.
Structure and text remain distinct facts. This protocol provides no type/call
resolution, data-flow joins, arbitrary functions, shell or network access.
Declared project/provenance facts must be available and bound before evaluation.

Complete empty collections give `all: true`, `any: false`, and `count: 0`.
Use an explicit count to reject emptiness. Parsing failure or incomplete facts
never become an empty collection. Required path facts are checked before
evaluation; a missing path cannot be hidden by filtering or a true branch.
All boolean operands and collection predicates share one budget, with no
short-circuit that can hide a capability gap or exhausted evaluation.

Limits per expression: depth 16, 256 expression nodes, 64 boolean operands or
membership values per list, 4096 bytes per string literal, 1024 bytes per regex,
64 KiB compiled regex and 256 KiB DFA cache. Evaluation accepts at most 50,000
entities and 32 MiB of aggregate name/text/path bytes, with 100,000 operations,
32 MiB cumulative string/regex scan bytes and a 30-second deadline. These limits
are shared across nested operations. Existing snapshot/AST acquisition budgets
still apply independently. Schema string lengths are also checked in UTF-8
bytes by semantic validation. Exhaustion stays incomplete with its reason;
fixed-assertion violations already found remain in the report.

Successful evaluation adds `metadata.expression` with the boolean result,
operation/scan counts, entity count and up to 64 false-subexpression observations.
Observations include expression pointers, entity index and available file/range.
Names/file previews are bounded to 256/1024 characters with explicit truncation;
omitted observations are counted. A false subexpression can be expected in an
`or` or `filter`; it is not independently a rule violation. A false expression emits an unlocated
aggregate violation, preserving collection-wide checks under delivery filtering.
The normal report binds policy, source review and snapshot; changes need recheck.

For packages containing v2, `rules validate` binds the exported
`schema project-rule-family` union of both pinned schemas, and v2 file entries
include their actual schema ID/digest. The family digest uses canonical compact
JSON bytes from the exported parsed document. V1-only reports retain their
existing schema identity. Generated candidates identify their actual v1 or v2
schema. Neither validation nor examples constitute independent approval.
