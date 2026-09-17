# 阶段 D：试点计划封存与启动完整性

> 日期：2026-09-16；状态：工程实现；需求：EVO-05-01、EVO-05-02、EVO-05-04。

## 交付契约

| 编号 | 交付及反例 |
|---|---|
| D-01 | `pilot seal --input` 只接受尚无 observation 的严格清单，并把封存后的完整 JSON 输出到 stdout；已有观察、未知字段、超限或符号链接输入均不产生封存结果 |
| D-02 | 封存前要求 owner、独立 reviewer、持久 archive、金额上限、seal/start/end、样本量和周期完整；时间戳和身份仍是调用方声明，不伪装成可信时间或签名 |
| D-03 | 封存清单只接收 real 来源、预先声明的 expected issues、无事后 exclusion 的 assignment；不同运行单元必须保持相同 task/base/snapshot/task digest 与真值身份 |
| D-04 | 至少两个请求模型组和 existing-tools/qualitygate 两种流程形成完整笛卡尔对照；每个 input 使用相同的计划 cohort 矩阵，拒绝缺组、重复组和不平衡任务 |
| D-05 | SHA-256 计划摘要绑定 schema、试点 ID、协议及全部 assignment；实际路由模型可在运行后补充，observation 可追加，其余计划字段变化使读取和汇总保持 incomplete |
| D-06 | `pilot summarize` 输出 seal 并把缺少 seal 列为协议限制；有效摘要只证明内容完整性，仍保持 `descriptive_only` 和 `requires_external_review` |

## 所有权与数据流

`domain::pilot` 定义 seal、规范化摘要输入和纯验证。摘要有意忽略
`cohort.actual_model`，因为服务商可能仅在执行后披露实际路由版本；请求模型、Agent、harness、
推理强度、流程、环境、工具、缓存和权限全部保留在封存输入中。

`config::pilot` 继续负责有界、无符号链接的清单读取；seal 不读取报告。
`application::pilot` 在 blocking worker 上读取、取当前时间并调用纯领域逻辑。
`interfaces` 只解析 `seal`/`summarize` 和渲染 JSON，不持有摘要或门禁决策。
依赖方向保持 domain ← config ← application ← interfaces。

## 声明边界

嵌入清单的摘要能够发现封存后对受保护计划字段的变化，但调用方仍可重新生成另一个摘要。
因此它不认证封存者、复核者、时间、归档服务或金额来源，也不构成试点接受或推广授权。
团队需要在外部受信流程中保存首次封存输出和其摘要，真实七天观察仍从该记录开始计算。

## 验证

纯领域测试覆盖幂等封存、实际模型后补、计划字段变化、缺治理字段、非真实来源、缺真值、
不完整矩阵和观察后封存。`pilot_seal` 集成测试覆盖 CLI 成功输出、部分观察汇总、计划篡改、
缺持久归档和晚封存。`pilot_summary` 保持报告摘要、长度、路径与缺失运行回归。
