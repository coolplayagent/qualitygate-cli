# 面向使用者 Agent 的行为契约

[English](../../en/02-reference/09-agent-contracts.md) · [本卷目录](README.md)

`agent.contracts.v1` 提供策略 `agent_obligations`、`schema agent-obligation` 导出和
只读 `agent-contract` 投影。指令直接作用于执行任务的 Agent。
[Skill 参考](../../../skills/qualitygate-cli/references/agent-contracts.md)包含完整结构、
预算、命令和采用流程。

每条义务绑定审核过的规范来源及解释，声明角色/任务范围、生命周期事件、必须/禁止行为，以及
指令、检查或宿主控制方式。投影保留快照、策略和任务身份，提供契约摘要、Prompt 及其摘要。
只有适用、已采用且来源仍有效的义务进入 Prompt。必需义务未采用、过期或缺少检查时保持未完成；
可选缺口继续列出。相同文本同时被要求和禁止时阻止就绪；语义冲突仍需审核。

投影不执行正式检查。`ready: true` 仅表示适合注入，不证明已经交付、理解或遵守。
检查引用只能证明相应检查支持的事实；宿主控制声明需要实际宿主实现。
该协议本身不提供任意工具拦截或 Agent 内部上下文恢复。Rust harness 接入属于后续实现阶段。

规范来源不能被排除，并保存在冻结策略包中。义务或检查定义改变会使采用记录失效；来源和固定策略
变化会使投影不再就绪。staged/diff 投影使用选定输入，保留与常规检查一致的策略信任边界。
普通代码门禁通过不能单独证明行为义务已满足。
