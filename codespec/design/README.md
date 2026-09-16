# Design

This directory is governed by `codespec/codespec-map.yaml`. Update its map entry through `relay-knowledge map directory` and keep reviewed source material within the declared content scope.

[Policy validation](../../docs/policy-validation.md) specifies the immutable
archive, protected operator inputs, shared snapshots, execution permits and
signed promotion/rollback boundaries. The [architecture contract](../../docs/architecture.md)
assigns these responsibilities to acyclic Rust owners.

[Phase A baseline design](pilot-phase-a.md) maps the cross-agent evolution
requirements to task preparation, comparable existing-tool records and
explicit pilot entry/exit criteria. Team-selected inputs remain separate from
controlled fixture evidence.

[Phase B design](pilot-phase-b.md) specifies bounded feedback, additive replay context,
external retry/evidence ownership and full checks after actual model changes.

[Phase C design](pilot-phase-c.md) specifies protected case provenance,
assignment-based pilot aggregation, report bindings, descriptive comparisons
and the boundary between engineering evidence and authorized rollout.
