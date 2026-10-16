# Agent 宿主与不可变交付

[English](../../en/02-reference/10-agent-host-and-delivery.md) · [本卷目录](README.md)

`agent.host.v1` 通过 `agent-run` 提供可复用 Rust 控制器；`examples/agent_loop.rs` 委托同一实现。
配置的 Agent 命令通过 stdin 接收当前已采用的契约、任务指令和反馈，每次尝试前都重新提供。
[Skill 宿主参考](../../../skills/qualitygate-cli/references/agent-host.md)定义参数、注入记录、
预算、恢复流程和信任条件。

## 验证与交付

宿主固定 CLI 身份、base 和策略 commit，独立执行完整任务检查并提供当前 Prompt。
修复、显式恢复和切换 Agent 都重新加载并注入义务；旧完成状态和 Agent 自报报告不能代替新证据。
必需宿主能力不可用时，在启动 Agent 前停止。完整检查及来源、策略、任务和范围绑定共同控制交付入口。

`agent.delivery.v1` 增加 `check --export-delivery <外部目录>`。只有通过的 full、无路径过滤检查
才能从同一份内存快照导出按内容寻址的 ZIP，包含未排除的上下文、未跟踪文件和执行位。
摘要由导出字节流计算，不从 Agent 提供的文件取得；检查后的工作区修改不会进入该对象。
宿主核验对象完整性，并授权该摘要对应的内容。接收者在使用前必须复核摘要。
输出目录必须位于规范化的仓库根目录之外；等价路径写法和 Windows 路径前缀不能绕过此边界。

## 支持的控制

支持的控制是本宿主入口上的 Prompt 注入、固定策略完整性、最终 full 检查和不可变交付。
任意工具授权、自然语言最终答复和 Agent 进程内部上下文恢复仍不可拦截。
注入记录只证明宿主提供了字节，不证明模型理解或遵守。必需的检查型义务仍拒绝 warning 级别违规；
其他 warning 继续保留在证据中。

调用者需要隔离可信控制器、执行器和证据，防止恶意操作系统访问。文件只读权限不等于可信保管，
内容寻址依赖使用时核验摘要。本宿主不是操作系统沙箱，不认证模型或操作者，也不意味着能强制控制
任意 Agent 的所有行为。


## 配置宿主

运行时需要声明 `agent.contracts.v1`、`agent.host.v1` 和 `agent.delivery.v1`。准备隔离检出目录、
仓库外已存在的证据目录、任务指令文件，以及从 stdin 接收指令的 Agent 命令 JSON 参数数组。
两个输入文件均为 UTF-8，分别不超过 16 KiB；参数数组包含 1–64 个参数，不隐含 Shell 插值。

以下是参数模板，执行前替换占位符：

```text
qualitygate --root <isolated-checkout> agent-run
  --qualitygate <absolute-matching-qualitygate>
  --base <base-commit> --policy-ref <authorized-policy-commit>
  --task <task.yaml> --role implementer
  --prompt-file <external-task.txt> --agent-command <external-agent.json>
  --output-dir <existing-external-directory>
  --max-attempts 3 --timeout-seconds 1800 --format json
```

宿主要求显式 base 和获准策略提交，暂不适配签名 active-policy 选择。独立验收需同时提供
`--trust-store` 和 `--evidence-dir`。每次调用允许 1–3 次尝试、1–1800 秒；连续两次诊断集合未严格
缩减时停止。

`--resume <previous-index.json>` 开始新的调用，使用新预算和新检查；Agent 命令变化时记录切换。
两者均不复用旧完成证据。`stdin_provided` 记录提供的字节，不证明模型理解。

底层 `check --export-delivery` 返回含 `report` 和 `delivery` 的 envelope；失败或未完成时没有交付
对象，quick 或路径过滤检查不能导出。ZIP 受快照预算、2 GiB 输出和 120 秒导出/摘要操作限制。
接收者使用前核对摘要，后续工作区编辑不属于该工件。

---

[上一章：面向使用者 Agent 的行为契约](09-agent-contracts.md) · [全书目录](../README.md) · [下一章：逻辑视图](../03-architecture/01-logical-view.md)
