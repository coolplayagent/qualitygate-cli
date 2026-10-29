# Qualitygate CLI handbook

[简体中文](../zh/README.md)

Qualitygate checks Git snapshots against repository policy and task acceptance.
This book moves from a first check to configuration, evidence and implementation.
Use the user guide in order; use the reference chapters when you need exact behavior.

## Choose a reading path

| Your goal | Start here | Continue with |
| --- | --- | --- |
| Run your first check | [Installation](01-user-guide/01-installation-and-first-check.md) | Snapshots, then rule configuration in Volume 1. |
| Turn repository instructions into rules | [Rule configuration](01-user-guide/03-rule-configuration.md) | Schema-guided authoring, then rules and coverage in Volume 2. |
| Integrate an Agent | [Repair and acceptance](01-user-guide/04-agent-repair-and-acceptance.md) | Agent contracts and host setup in Volume 2. |
| Change Qualitygate itself | [Development](04-contributor-guide/01-development-and-quality-gates.md) | Source ownership in Volume 3 and test evidence in Volume 4. |

## Contents

### [1. User guide](01-user-guide/README.md)

Complete a first check, understand its evidence, configure rules and finish an Agent repair cycle.

1. [Installation and first check](01-user-guide/01-installation-and-first-check.md)
2. [Snapshots and the check workflow](01-user-guide/02-snapshots-and-check-workflow.md)
3. [Rule configuration](01-user-guide/03-rule-configuration.md)
4. [Schema-guided repository rules](01-user-guide/06-schema-guided-repository-rules.md)
5. [Agent repair and acceptance](01-user-guide/04-agent-repair-and-acceptance.md)
6. [Evaluating adoption in a real repository](01-user-guide/05-real-repository-pilot.md)

### [2. Reference](02-reference/README.md)

Look up commands, rule protocols, reports, policy authority, runtime limits and Agent interfaces.

1. [CLI commands](02-reference/01-cli-commands.md)
2. [Built-in and custom rules](02-reference/02-rules.md)
3. [Projects and tool reports](02-reference/03-projects-and-tool-reports.md)
4. [Policy, task, and signed evidence](02-reference/04-policy-task-and-signed-evidence.md)
5. [Exit codes and resource limits](02-reference/05-exit-codes-and-resource-limits.md)
6. [Runtime capabilities and environment preflight](02-reference/06-runtime-and-preflight.md)
7. [Constraint coverage](02-reference/07-constraint-coverage.md)
8. [Consuming-agent contracts](02-reference/09-agent-contracts.md)
9. [Agent host and immutable delivery](02-reference/10-agent-host-and-delivery.md)

### [3. Architecture — 4+1 views](03-architecture/README.md)

Understand the domain model, execution flow, source ownership and delivery boundaries.

1. [Logical view](03-architecture/01-logical-view.md)
2. [Process view](03-architecture/02-process-view.md)
3. [Development view](03-architecture/03-development-view.md)
4. [Physical view](03-architecture/04-physical-view.md)
5. [+1 Scenarios](03-architecture/05-scenarios.md)
6. [Rule engine implementation](03-architecture/06-rule-engine-implementation.md)
7. [Rule capabilities and Agent behavior contracts](03-architecture/07-rule-capabilities-and-agent-contracts.md)

### [4. Contributor guide](04-contributor-guide/README.md)

Build and verify the project, maintain fixtures and trace requirements to evidence.

1. [Development and quality gates](04-contributor-guide/01-development-and-quality-gates.md)
2. [Selfcheck and fixtures](04-contributor-guide/02-selfcheck-and-fixtures.md)
3. [Requirement-to-test evidence](04-contributor-guide/03-requirements-to-test-evidence.md)
4. [Pilot evaluation](04-contributor-guide/04-pilot-evaluation.md)
