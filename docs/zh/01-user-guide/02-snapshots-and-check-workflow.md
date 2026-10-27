# 快照与检查流程

[English](../../en/01-user-guide/02-snapshots-and-check-workflow.md) · [本卷目录](README.md)

## 理解检查输入

| 概念 | 确定的内容 |
| --- | --- |
| 快照 | 执行使用的精确文件字节及 Git 变更选择。 |
| 策略 | 本次采用的规则、命令检查、严重级别和 profile。 |
| 任务契约 | 针对某次请求附加的验收项，可按需提供。 |
| 证据 | 绑定上述输入的结果和工件；输入变化后需要重新验证。 |

`quick` 和 `full` 选择要运行的检查；快照选择器决定检查哪些字节和变更。
因此 full 不自动代表全仓库诊断：CLI 验证的是 Git 变更选择出的交付范围。

## 选择快照

应选择与交付面一致的一种快照选择器：

```bash
qualitygate check --staged --profile full --format json
qualitygate check --worktree --profile quick --format json
qualitygate check --diff BASE..HEAD --profile full --format json
qualitygate check --mr https://github.com/owner/repository/pull/123 --format json
```

- `--staged` 读取索引区字节，而不是工作区副本。
- `--worktree` 捕获当前工作区以及符合范围的未跟踪文件。
- `--diff` 比较两个已解析提交。
- `--mr` 解析 GitHub 或 GitLab 合并请求，并记录实际 base、head、merge base、平台与获取证据。

MR 使用调用方提供的凭据和端点。认证失败、网络问题、ref 漂移、限流或不支持的平台响应都属于
未完成执行，不能解释为干净结果。

## 计划、执行与决策

CLI 从选中快照（或显式策略引用）严格加载配置，验证可选任务契约，映射改动和项目事实，
再生成带依赖关系的检查计划。命令在快照的一次性物化目录中运行；标准输出、标准错误、状态、
耗时、工具版本和报告工件均受限采集。

`quick` 和 `--path` 会主动省略部分工作，并列出未执行的交付检查。只有无路径过滤的 `full`
报告，同时满足范围为 `delivery` 或 `task`、待执行交付项为空且 gate 完整，才可给出已配置的
交付结论。

## 将报告作为证据阅读

| 字段 | 需要确认的内容 |
| --- | --- |
| `snapshot` 与 `policy` | 结果来自哪份采集输入和策略。 |
| `selection` | 变更文件/行、排除项、空交付和执行上下文。 |
| `profile` 与 `scope` | 运行了哪些检查，属于交付、任务还是路径反馈。 |
| `plan.pending_delivery_checks` | 做出交付结论前仍需完成的工作。 |
| `gate.complete` 与 `gate.decision` | 验证是否完成，是否仍有阻塞项。 |

行级诊断适用于变更行；无行号的诊断按变更文件判定，包括删除、重命名、二进制和权限变化。
无位置命令失败、超时和证据缺失仍影响门禁。空交付不能证明全仓质量。

内容完全相同的重命名按路径字典序，将已删除路径与新增路径一一配对。
每个旧路径只能作为一次重命名的来源；多出的相同副本属于新增文件，照常检查新增文本行和新文件规则。
未配对的旧路径仍属于删除；保留在目标快照中的原文件不能作为重命名来源。
二进制和空文件保持变更文件语义，不虚构行位置。重命名匹配比较文件字节，不比较可执行权限。

将 JSON 报告与引用证据一起保存。内容身份和验证范围分别绑定，详见
[证据绑定](../02-reference/04-policy-task-and-signed-evidence.md)。文件、策略、任务或报告输入变化后，
重新捕获并检查快照。

## 处理大型仓库

即使 diff 很小，构建和测试仍需要未排除的依赖上下文；语言选择和 `--path` 不会移除这些输入。
先对实际选择器执行 `doctor`，通过元数据确认容量或不支持文件带来的缺口。

1. 保留检查需要的文件，有依据时选择受支持的采集预算。
2. 对无关资源审核精确策略排除项；这些文件也会从命令输入中消失。
3. 使用相同选择器和预算重新执行 doctor 与最终 full 检查。

例如，报告提示需要单文件 4 MiB 时：

```bash
qualitygate doctor --worktree --profile full --snapshot-max-file-mib 4 --format json
qualitygate check --worktree --profile full --snapshot-max-file-mib 4 --format json
```

排除语法、受保护文件和所选策略行为见[运行时与预检](../02-reference/06-runtime-and-preflight.md)，
全部预算选项见[资源限制](../02-reference/05-exit-codes-and-resource-limits.md)。提高容量只影响采集，
不代表策略获批。


---

[上一章：安装与首次检查](01-installation-and-first-check.md) · [全书目录](../README.md) · [下一章：规则配置](03-rule-configuration.md)
