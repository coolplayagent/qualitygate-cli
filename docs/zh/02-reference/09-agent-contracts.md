# 面向使用者 Agent 的行为契约

[English](../../en/02-reference/09-agent-contracts.md) · [本卷目录](README.md)

`agent.contracts.v1` 提供策略 `agent_obligations`、`schema agent-obligation` 导出和
只读 `agent-contract` 投影。指令直接作用于执行任务的 Agent。
[Skill 参考](../../../skills/qualitygate-cli/references/agent-contracts.md)包含完整结构、
预算、命令和采用流程。

## 义务与就绪状态

每条义务绑定审核过的规范来源及解释，声明角色/任务范围、生命周期事件、必须/禁止行为，以及
指令、检查或宿主控制方式。投影保留快照、策略和任务身份，提供契约摘要、Prompt 及其摘要。
只有适用、已采用且来源仍有效的义务进入 Prompt。必需义务未采用、过期或缺少检查时保持未完成；
可选缺口继续列出。相同文本同时被要求和禁止时阻止就绪；语义冲突仍需审核。

投影不执行正式检查。`ready: true` 仅表示适合注入，不证明已经交付、理解或遵守。
检查引用只能证明相应检查支持的事实；宿主控制声明需要实际宿主实现。
该协议本身不提供任意工具拦截或 Agent 内部上下文恢复。[Rust 宿主](10-agent-host-and-delivery.md)
现已注入这些投影，并强制执行其明确支持的控制。

## 失效与复验

规范来源不能被排除，并保存在冻结策略包中。义务或检查定义改变会使采用记录失效；来源和固定策略
变化会使投影不再就绪。staged/diff 投影使用选定输入，保留与常规检查一致的策略信任边界。
普通代码门禁通过不能单独证明行为义务已满足。


## 获取并使用投影

```bash
qualitygate capabilities --format json
qualitygate schema agent-obligation --format json
qualitygate agent-contract --worktree --role implementer --format json
```

需要 `agent.contracts.v1`，不能仅凭版本号推断支持。若验证选用了 `--base`、`--policy-ref` 和
`--task`，投影也应使用相同输入。支持 staged/diff 投影。读取返回的 `prompt`，保存契约与 Prompt
摘要，在 Agent 行动前注入；恢复、切换 Agent 或相关输入变化后重新加载。

| 约束方式 | 证据边界 |
| --- | --- |
| `instruction` | 提供行为指导；送达本身不证明遵守。 |
| `checks` | 引用配置的检查，只证明这些检查支持的事实。 |
| `host` | 声明所选宿主必须实际实现的控制。 |

退出码 `0` 且 `ready: true` 表示可以注入；必需缺口或精确指令冲突返回 `ready: false` 和退出码 `2`。
空角色/任务范围匹配全部角色和任务，任务范围匹配 task ID 而非文件名。最多 128 条义务、每列表
32 个角色/任务、每种极性 16 条指令、64 个检查引用，投影 Prompt 上限为 512 KiB。

编写时通过 `rules source` 获取精确来源绑定。`config --show` 返回 `agent_bindings`，由获准审核者
独立记录采用。义务或映射检查设置变化后绑定失效。采用记录不能授予 Agent 权限或覆盖宿主指令。

---

[上一章：有界算子规则](08-bounded-operators.md) · [全书目录](../README.md) · [下一章：Agent 宿主与不可变交付](10-agent-host-and-delivery.md)
