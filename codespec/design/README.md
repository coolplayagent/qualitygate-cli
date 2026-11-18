# Design

This directory is governed by `codespec/codespec-map.yaml`. Update its map entry through `relay-knowledge map directory` and keep reviewed source material within the declared content scope.

The [tool facts and assertions design](tool-facts-and-assertions.md) describes how a project checker
returns diagnostics or publishes typed facts for V4 rules, which runtime capabilities it needs,
and how the offline bridge is distributed. Its [requirements](../requirements/tool-facts-and-assertions.md)
define TFA-01–TFA-08. The [rule engine and agent-built tool review](rule-engine-and-tool-builders.md)
records the fixture outcomes and the open producer-side validation gap for clone pairs.

[Policy validation](../../docs/en/02-reference/04-policy-task-and-signed-evidence.md) specifies the immutable
archive, protected operator inputs, shared snapshots, execution permits and
signed promotion/rollback boundaries. The [architecture contract](../../docs/en/03-architecture/03-development-view.md)
assigns these responsibilities to acyclic Rust owners.

The [pilot design](pilot.md) brings together the current v10 trial, its closeout
conditions and phases A–O. Start with the [trial protocol](pilot.md#本轮协议与收尾条件)
for inputs, responsibilities and outstanding evidence. Use the [version guide](pilot.md#版本与证据导航)
when replaying a previously sealed manifest.
