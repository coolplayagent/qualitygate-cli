# Qualitygate CLI 使用与开发手册

[English](../en/README.md)

Qualitygate 根据仓库策略和任务验收要求检查 Git 快照。本书从首次检查开始，逐步介绍配置、证据和
实现原理。用户指南适合顺序阅读；需要精确行为时，查阅参考手册中的对应章节。

## 选择阅读路线

| 你的目标 | 从这里开始 | 接着阅读 |
| --- | --- | --- |
| 完成第一次检查 | [安装与首次检查](01-user-guide/01-installation-and-first-check.md) | 第一卷的快照与规则配置。 |
| 将仓库规范转为规则 | [规则配置](01-user-guide/03-rule-configuration.md) | Schema 编写流程，再查第二卷的规则与条款覆盖。 |
| 接入 Agent | [修复与验收](01-user-guide/04-agent-repair-and-acceptance.md) | 第二卷的 Agent 契约与宿主接入。 |
| 修改 Qualitygate | [开发与质量门禁](04-contributor-guide/01-development-and-quality-gates.md) | 第三卷的源码所有权、第四卷的测试证据。 |

## 全书目录

### [1. 用户指南](01-user-guide/README.md)

完成首次检查，理解证据，配置规则，并走通 Agent 修复验收。

1. [安装与首次检查](01-user-guide/01-installation-and-first-check.md)
2. [快照与检查流程](01-user-guide/02-snapshots-and-check-workflow.md)
3. [规则配置](01-user-guide/03-rule-configuration.md)
4. [Schema 驱动的仓库规则](01-user-guide/06-schema-guided-repository-rules.md)
5. [Agent 修复与验收](01-user-guide/04-agent-repair-and-acceptance.md)
6. [在真实仓库评估采用效果](01-user-guide/05-real-repository-pilot.md)

### [2. 参考手册](02-reference/README.md)

查阅命令、规则协议、报告、策略权威、运行时限制和 Agent 接口。

1. [CLI 命令](02-reference/01-cli-commands.md)
2. [内置与自定义规则](02-reference/02-rules.md)
3. [项目与工具报告](02-reference/03-projects-and-tool-reports.md)
4. [策略、任务与签名证据](02-reference/04-policy-task-and-signed-evidence.md)
5. [退出码与资源限制](02-reference/05-exit-codes-and-resource-limits.md)
6. [运行时能力与环境预检](02-reference/06-runtime-and-preflight.md)
7. [条款覆盖](02-reference/07-constraint-coverage.md)
8. [有界算子规则](02-reference/08-bounded-operators.md)
9. [面向使用者 Agent 的行为契约](02-reference/09-agent-contracts.md)
10. [Agent 宿主与不可变交付](02-reference/10-agent-host-and-delivery.md)

### [3. 架构——4+1 视图](03-architecture/README.md)

理解领域模型、执行流程、源码所有权和交付边界。

1. [逻辑视图](03-architecture/01-logical-view.md)
2. [进程视图](03-architecture/02-process-view.md)
3. [开发视图](03-architecture/03-development-view.md)
4. [物理视图](03-architecture/04-physical-view.md)
5. [+1 场景视图](03-architecture/05-scenarios.md)
6. [规则引擎实现](03-architecture/06-rule-engine-implementation.md)
7. [规则能力与 Agent 行为契约](03-architecture/07-rule-capabilities-and-agent-contracts.md)

### [4. 贡献者指南](04-contributor-guide/README.md)

构建并验证项目，维护 fixtures，将需求追溯到测试证据。

1. [开发与质量门禁](04-contributor-guide/01-development-and-quality-gates.md)
2. [自检与 fixtures](04-contributor-guide/02-selfcheck-and-fixtures.md)
3. [需求到测试证据](04-contributor-guide/03-requirements-to-test-evidence.md)
4. [试点评估](04-contributor-guide/04-pilot-evaluation.md)
