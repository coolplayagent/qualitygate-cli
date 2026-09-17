# 阶段 G：封存结构化总预算并纳入独立验收

> 日期：2026-09-16；需求：EVO-05-01、EVO-05-03；状态：工程实现，真实试点未启动。

## 交付契约

| 编号 | 要求与反例 |
|---|---|
| G-01 | 新试点采用 manifest schema v2，在观察前固定币种、计价时间、模型/基础设施价格来源、人工时薪和 32 个运行单元共享的总金额上限；缺字段、零/越界值、非三位大写币种或同时提供旧自由文本上限必须拒绝封存 |
| G-02 | plan seal 绑定结构化预算；改动金额、币种、人工单价或来源使旧封存及启动授权失配。schema v1 已封存记录保持原摘要与九项阈值语义，不伪装成 v2 自动预算验收 |
| G-03 | 纯领域层对所有 assignment 的全部尝试累计模型与基础设施费用，失败、超时和放弃尝试仍计入；每条 observation 的人工活跃毫秒按封存时薪向上取整为微币单位 |
| G-04 | 缺 observation、零尝试、缺成本分量、缺人工时间或币种/计价时间不一致为 unknown；已知部分超过上限时即为 not_met，不能被其它未知项掩盖 |
| G-05 | v2 验收主题使用 `qualitygate-pilot-thresholds-v2`，增加第十项 `total_cost_micros` 检查，绑定汇总成本与来源 manifest。超预算或未知总成本不能签 accepted；签署 rejected 仍合法 |
| G-06 | `pilot summarize` 显示模型、基础设施、人工已知成本及缺口数量；无来源的价格和人工计时仍为外部声明，签名不证明真实账单或因果收益 |

## 数据与兼容性

`templates/pilot/observation-v2.json` 是阶段 G 的原始模板；阶段 H 起的新试点使用
`templates/pilot/observation-v3.json`。`protocol.budget` 包含 `currency`、
`priced_at`、`source`、`max_total_micros` 和 `human_hourly_micros`；一个微币单位是币种的
百万分之一。v2 的 `monetary_cap` 必须为 null，避免两个上限冲突。v1 继续使用原字段、摘要、
九项评估和人工预算判断，不在读取旧封存记录时重新封存或修改签名主题。

预算采用整数累加与检查过的算术；人工费用逐 observation 计算
`ceil(review_active_ms × human_hourly_micros / 3_600_000)`。总额跨两种模型与两条流程，
包括所有尝试，不用合格交付数作分母。既有 `cost_ratio` 仍比较每个匹配组的模型/基础设施
成本，不包含人工；第十项另行检查全试点总额。

## 所有权与验收边界

`domain::pilot::validation` 验证版本化预算并将其纳入已有 plan seal；
`domain::pilot::budget` 纯计算，`metrics` 输出成本，`acceptance` 构建第十项检查。
`config` 沿用 1 MiB manifest 与 64 MiB report 上限，`application` 和 `interfaces` 沿用
阶段 F 的签名、外部输入与发布流程。无新网络、进程或签名密钥所有者。

固定测试 key、合成金额和复核分钟只证明实现。正式上限、币种、定价来源、人工时薪与真实任务
必须由团队在封存前确定；缺少实际账单或人工计时，不能把测试汇总当作生产收益。

## 测试

领域测试覆盖 v1 摘要兼容、v2 封存和预算漂移、失败尝试费用、人工向上取整、缺值与已知超限并存；
验收领域测试覆盖第十项 met/not_met/unknown；CLI 集成覆盖 v2 独立签名接受及成本比例仍通过、
但全试点超额时拒绝接受。阶段 F 的 v1 adapter 和反例继续运行。
