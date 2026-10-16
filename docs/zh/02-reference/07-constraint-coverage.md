# 条款覆盖

[English](../../en/02-reference/07-constraint-coverage.md) · [分册索引](README.md)

能力 `constraints.coverage.v1` 为策略增加可选的 `constraints` 清单，独立于现有
project-rule v1 DSL。通过 `rules source` 获取来源，通过 `qualitygate schema constraint`
导出单条记录的 Schema。完整编写及采用流程见[打包参考](../../../skills/qualitygate-cli/references/constraint-coverage.md)。

## 条款记录与采用

每条记录包含身份/版本、精确章节及摘要、解释、必需性、实现映射和正反例引用。
`checks` 实现引用策略中显式配置的检查；`unsupported` 实现记录原因及证据负责人。
五种机制标签只是分类，不会凭标签增加算子或 Agent 宿主拦截能力。例子引用是审核输入，
不冒充本次运行证据。适用范围由映射检查的定义及报告的选择范围决定。

只读 `config --show --format json` 返回 `constraint_bindings`，同时绑定条款和所引用检查的
有效定义。获得授权后才能写入对应采用记录；复制摘要不等于独立批准。
`local_candidate`、`caller_supplied_ref` 和签名活动策略保持原有信任边界。检查定义修改会使采用记录过期。

## 执行状态

非空清单自动增加最终的保留检查 `constraint-coverage`。报告保留全部条款及快照、策略、规则和任务摘要。
必需条款不支持、未采用、采用过期或执行证据缺失时为 `incomplete`；映射检查完整执行且违规时为 `fail`。
判定前检查全部引用，失败不能掩盖另一项证据缺失。可选缺口继续可见；必需条款不会因为引用检查为 warning
就退化为建议。已验证条款继承实际检查范围，包括 delivery/path 限制。

规范来源不可被排除，并会保留在冻结策略包中。工作区变更不影响 staged 检查；策略或来源字节改变后，
不能复用另一快照的证据。来源缺失、章节歧义和摘要变化都保持未完成。quick 可能留下未运行的映射命令；
交付必须使用最终 full 检查。

## 限制与解释边界

限制为 256 条条款、每条 64 项检查、每种正反例各 16 项，以及现有 1 MiB 策略预算。
未知字段、重复项、空映射、未知策略检查、自引用及非法来源会被拒绝。覆盖只描述声明的清单，
不能证明自然语言提取已穷尽或解释充分。[有界算子协议](08-bounded-operators.md)已单独提供；
[Agent 契约](09-agent-contracts.md)和 [Rust 宿主](10-agent-host-and-delivery.md)分别通过能力声明提供对应接口。


## 查看覆盖情况

```bash
qualitygate capabilities --format json
qualitygate schema constraint --format json
qualitygate config --show --format json
qualitygate check --worktree --profile full --format json
```

运行时应声明 `constraints.coverage.v1`，仅凭包版本号不能确认支持。采用前审核
`constraint_bindings`，检查后查看自动检查项中的 `metadata.constraint_coverage`。

| 条款状态 | 含义 |
| --- | --- |
| `unsupported` | 尚未映射受支持的证据机制。 |
| `unadopted`、`stale_adoption`、`source_invalid` | 审核或来源绑定缺失、失效。 |
| `pending` | 映射检查缺失、未选择、禁用或未完成。 |
| `failed` | 完整映射证据包含违规。 |
| `verified` | 映射检查在报告主体和范围内通过。 |
| `not_applicable` | 映射检查明确报告不适用。 |

---

[上一章：运行时能力与环境预检](06-runtime-and-preflight.md) · [全书目录](../README.md) · [下一章：有界算子规则](08-bounded-operators.md)
