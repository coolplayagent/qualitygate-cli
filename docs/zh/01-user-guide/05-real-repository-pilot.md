# 在真实仓库评估采用效果

[English](../../en/01-user-guide/05-real-repository-pilot.md) · [本卷目录](README.md)

完成一次检查与修复后，可以进一步评估 Qualitygate 对团队的实际帮助：拦截了哪些问题、
增加了多少噪声、是否改善修复和审查效率。门禁通过本身不能回答这些问题。

## 按目标选择证据

| 目标 | 需要的证据 | 继续阅读 |
| --- | --- | --- |
| 在仓库中采用检查 | 审核过的策略、工具前置条件和 full 报告 | [规则配置](03-rule-configuration.md) |
| 验证安装或排查回归 | fixture 结果与独立 golden 对照 | [自检与 fixtures](../04-contributor-guide/02-selfcheck-and-fixtures.md) |
| 与现有工具比较收益 | 可比的真实任务、完整尝试记录和独立复核 | [试点评估](../04-contributor-guide/04-pilot-evaluation.md) |

## 准备试点

先确定真实任务、复核者和衡量标准，再收集结果。失败尝试、超时和证据缺失也应进入记录。
需要封存计划、外部授权和独立验收时，采用版本化试点协议；执行矩阵与签名要求统一见
[评估流程](../04-contributor-guide/04-pilot-evaluation.md)。

日常仓库检查无需先开展试点。fixtures 和历史回放可用于验证工程机制，但不能证明真实使用收益。

---

[上一章：Agent 修复与验收](04-agent-repair-and-acceptance.md) · [全书目录](../README.md) · [下一章：CLI 命令](../02-reference/01-cli-commands.md)
