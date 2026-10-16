# Bounded operator rules

[简体中文](../../zh/02-reference/08-bounded-operators.md) · [Volume index](README.md)

Capability `rules.operators.v2` introduces project-rule `schema_version: 2` with
a required typed `expression`. Existing v1 schema and semantics remain unchanged.
Export `schema operator-rule`; default `rules schema` still describes v1.
The [packaged operator reference](../../../skills/qualitygate-cli/references/operators.md)
contains the full operation set, example, limits and authoring workflow.

## Expression semantics

Expressions compose existing text/AST entity facts with boolean logic,
`all/any/filter/count`, typed comparison, membership and regex. Fields are
lexically bound `name`, `text` and `path`; no arbitrary code or dynamic fields
run. Existing fixed assertions remain additive, including marker retention and
dependency checks. Expression collections contain triggered entities selected
by the rule, and do not expand the adapter's semantic knowledge.

Type/shape validation covers every branch. Fact collection must complete before
evaluation; declared dependency/provenance inputs must retain their binding.
Complete empty sets have defined semantics: all is true, any false and count
zero. Explicit counting prevents vacuous success when at least one entity is
required. Parse failures, missing facts and exhausted budgets stay incomplete.
Earlier fixed-assertion findings are retained even if the expression cannot finish.

## Evaluation bounds and reports

One evaluation budget covers nested operations: 256 nodes, depth 16, 50,000
entities, 32 MiB of facts, 100,000 operations, 32 MiB scanned strings and a
30-second deadline. Literal/regex compilation bounds are separate and strict.
No boolean shortcut hides a missing fact or expensive branch. Existing acquisition
budgets still apply; no additional general-purpose execution runtime is introduced.

The report preserves the original snapshot, policy and source-review binding.
Successful evaluation includes operation/scan metrics; false expressions produce
aggregate violations. Mixed authoring packages identify the exported family
schema and each v2 file's actual contract. Schema validity and local source
reviews remain distinct from independent policy approval and logical adequacy.


## Author and validate an expression

Confirm `rules.operators.v2` in `capabilities`; export `schema operator-rule`
for the complete v2 shape. This fragment requires at least one selected entity
and checks every selected name. Combine it with `when.entity: test_method`, the
`test_methods` capability, identity, source binding and the other required fields:

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

Save the complete rule as `candidate.yaml`, run `rules validate candidate.yaml`,
then `rules generate --input candidate.yaml` within the authorized authoring
workflow. V1 rejects `expression`; generation does not enable the rule.

---

[Previous: Constraint coverage](07-constraint-coverage.md) · [Contents](../README.md) · [Next: Consuming-agent contracts](09-agent-contracts.md)
