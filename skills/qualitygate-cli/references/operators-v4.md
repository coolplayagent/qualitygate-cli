# V4: typed producer facts

This reference describes the development runtime's `rules.operators.v4`
capability. Confirm it with `capabilities` before using these contracts;
an older published package may not support it. Export the matching schema
with `schema operator-rule-v4`. V1–V3 schemas and interpretation are unchanged.

## Tool observations

Require a complete producer collection bound to the selected snapshot,
explicit project/target/configuration and declared measurement definition.
The producer passing means collection completed; the consumer assertion
decides whether the observed values meet the repository policy.
Missing, stale, unexecuted or incomplete facts cannot become an empty pass.
Measurement thresholds apply to serialized tool observations, including any
documented rounding performed by that tool; they do not recover lost precision.

## Rule shape

V4 uses `assertion` instead of the legacy `expression` key. Existing `sources`
still select source entities; `fact_sources` names producer collections. These
maps share one namespace with a combined maximum of 16 names; `entities` is
reserved for the primary source selector. The existing `when`, `applies_to`
and fixed `then` assertions retain their behavior.

A fact source declares `producer`, `schema`, `schema_version: 1`, `project`,
optional exact `target` and `configuration`, plus `unit` and `measurement`.
`function_metric` requires a `metric` selector. `clone_pair` forbids that key
and uses `unit: ratio`. The selected policy rule must explicitly list the
producer in `depends_on`; the producer declares a matching `tool_facts` output.
The application verifies those declarations against actual execution evidence.
Rule producers must be declared in the policy's `checks`; the existing policy
dependency graph does not resolve producers supplied only by task verification.

Tool-facts protocol v1 has a fixed Rust syntax inventory: function items with a
body, excluding closures, block-local functions and functions in modules named
`tests`. Non-Rust inputs, ambiguous identities and unsupported generic/scoped
impl self types reject acquisition. These exclusions define the collection
universe; `all` on its empty inventory says nothing about excluded entities.

Metric fields are `id`, `name`, `path`, `project`, optional `target` and
`configuration`, `metric`, `value`, `unit` and `measurement`. Clone fields
replace the function fields with `left_path`, `right_path`, `left_symbol`,
`right_symbol` and `similarity`. Missing accessed fields block evaluation.
Fields not registered for that source type are rejected before execution.

Clone sources declare their collection universe through `minimum_lines`
(default 1), `minimum_nodes` (default 0) and `similarity_floor` (default zero).
The acquired universe must be at least as broad. An acquisition floor of 0.82
cannot satisfy a source requesting 0.80. A source explicitly requesting 0.82
can support conclusions only about that restricted collection; it cannot
establish absence of pairs between 0.80 and 0.82. Output retains the declaration.

Pinned dryer filters the raw score before rounding its output to twelve decimal
places. Its acquisition floor must therefore also cover scores that round up
to the requested boundary: collect at zero, or below the exact rounding boundary
checked by the adapter contract. For a source floor of 0.82, acquisition at
0.819999999999 is sufficient; acquisition at 0.82 is rejected. This collection
guard does not change the exact decimal comparisons used by assertions.

## Values and operations

V4 retains finite lexical `all`, `any`, `filter`, `map`, `count`, `distinct`,
`contains`, `subset`, boolean/string operations and comparisons. No user code,
I/O, arbitrary JSON field access, recursive functions or numeric coercion from
strings is available. Numeric fields from different sources must agree on
fact schema, unit and measurement definition. Numeric literals are thresholds
in the compared field's declared units; no unit conversion is performed.
Collection counts compare with other counts or numeric literals. A computed
count cannot implicitly become a measured score, percentage or ratio.

`number` contains an i64. `decimal` contains a canonical object, for example
`{coefficient: '81', scale: 2}` for 0.81. A coefficient has at most 38 digits,
scale is 0..18, zero is `{coefficient: '0', scale: 0}`, and removable trailing
zeros or negative zero are rejected. Comparisons are exact, including integer
promotion. Unsupported precision, nonfinite values and overflow never round
into a passing result. `in` supports corresponding typed scalar values.

All declared collections must be complete, even when a boolean operand would
otherwise short-circuit. For a verified empty collection, `all` is true,
`any` is false and `count` is zero. Require `count >= 1` when empty inventories
are unacceptable. A single producer's complete result can serve multiple rules
in one run; each rule retains its own verdict and work budget.

The limits are 50,000 input/intermediate records, 32 MiB retained facts,
32 MiB scanned bytes, 100,000 operations, 256 expression nodes and depth 16.
In-process collection and evaluation have separate 30-second limits; external
commands retain their configured timeouts. Exceeding any limit is incomplete.
False observations retain source identities and both locations for clone pairs.

## Example and adoption

[The metric example](examples/operators-v4.yaml) binds the Tool observations
section above. Its `scan` producer and `cc-v1` measurement are illustrative
generic producer contracts, not implicit native-tool names or production
threshold recommendations. Configure a matching producer, retain independent
source inventory evidence, run source/schema validation and adopt the rule
through the repository's existing policy lifecycle.

Native crapper/dryer adapters have their own pinned measurement definitions;
copy those actual definitions rather than relabeling them `cc-v1`. Mutator
does not supply strict acceptance evidence in this stage, and no model-checker
or viewer integration is implied by V4 support.
