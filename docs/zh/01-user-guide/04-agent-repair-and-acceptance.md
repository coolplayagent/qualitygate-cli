# Agent 修复与验收

[English](../../en/01-user-guide/04-agent-repair-and-acceptance.md) · [本卷目录](README.md)

Qualitygate 反馈为 Agent 提供具体问题和可复验命令。整个修复过程应明确仓库策略、任务验收项和
重试预算。

## 获取修复反馈

```bash
qualitygate check --worktree --profile quick --feedback --format json
```

有界反馈包含诊断 ID、位置、证据引用、限制、复验参数、待执行交付检查及原报告身份。
将原报告与反馈一起保存。quick 结果用于指导编辑，不授权交付或修改策略。

策略声明了 Agent 行为义务时，应先获取并应用所选角色和任务的
[Agent 契约](../02-reference/09-agent-contracts.md)。恢复、切换 Agent 或相关输入变化后重新加载。

## 执行有限修复循环

1. 保存初始快照、选中策略、任务和 full 报告。
2. 向 Agent 提供相关反馈及获准修改的源码范围。
3. 记录每次尝试，包括失败与超时。
4. 使用相同策略和任务权威复查变更。
5. 成功、次数或时间耗尽、连续无进展、无法恢复前置条件时停止。
6. 对实际组合后的最终快照执行无路径过滤的 full 检查。

次数与耗时预算由宿主管理。修复不能关闭必需规则、降低严重级别、替换受信策略或伪造人工批准。
快照变化后必须生成新报告。

| 结果 | 处理方式 |
| --- | --- |
| 阻塞违规 | 在获准源码范围内修复，再复验。 |
| 执行未完成 | 恢复缺失的工具、输入或证据；无法恢复时保留缺口。 |
| full 完整通过 | 核对范围、绑定和独立验收后再交付。 |
| 预算耗尽或无进展 | 停止并报告剩余问题与未完成项。 |

## 完成任务验收

```bash
qualitygate check --worktree --profile full --task task.yaml --format json
```

存在任务契约时使用 `--task`；每次复验都保留已选定的策略与信任输入。任务契约在仓库策略上叠加
可观察验收项。命令验收必须证明目标测试实际执行，零测试不能算通过；必需的人工验收需要外部
签名证据，详见[策略、任务与签名证据](../02-reference/04-policy-task-and-signed-evidence.md)。

交付通过报告应满足 `profile: full`、`scope: delivery` 或 `task`、
`plan.pending_delivery_checks` 为空、`gate.complete: true` 且 `gate.decision: pass`。
同时保留警告和已知限制。

## 使用 Rust 宿主管理交付

可复用的 [`agent-run` 宿主](../02-reference/10-agent-host-and-delivery.md)注入当前义务，独立运行 full
检查，并授权不可变交付工件。选择声明了所需能力的运行时，显式配置 Agent 命令。
宿主参考介绍接入方式和支持的控制；指令送达回执本身不能证明遵守情况，也不代表控制所有 Agent 行为。

---

[上一章：Schema 驱动的仓库规则](06-schema-guided-repository-rules.md) · [全书目录](../README.md) · [下一章：在真实仓库评估采用效果](05-real-repository-pilot.md)
