# 阶段 F：认证独立试点验收

> 日期：2026-09-16；状态：工程实现；需求：EVO-05-03、EVO-05-04。

## 交付契约

| 编号 | 交付及反例 |
|---|---|
| F-01 | `pilot acceptance-subject` 只为完整、已过观察窗口、计划与启动授权均有效的真实试点生成验收主题；缺运行、缺报告、未知实际模型或未认证启动均保持未完成 |
| F-02 | 纯领域评估聚合 Qualitygate 组的检测率、误报率、修复率、必需检查完成率和复核比例，并要求每个模型/任务对照的 inventory、审查时间、full P95 和成本比全部为真 |
| F-03 | 验收主题绑定 repository、pilot、plan seal、完整 manifest 摘要、启动授权记录/签名 key 和阈值评估详情摘要；observation、report 引用、成本或人工数据变化使旧主题失配 |
| F-04 | 最终记录使用独立 payload type；只有协议 reviewer 对应、具备 `pilot-trial-acceptance` scope 的 human Ed25519 key 可以签署，owner 自批、错 key、错仓库、过期和撤销均拒绝 |
| F-05 | `accepted` 只允许所有 9 项检查均为 `met`；任一 `not_met` 或 `unknown` 都不能签成接受。独立 reviewer 可以对完整证据签署 `rejected` |
| F-06 | `pilot summarize --acceptance` 每次重验启动和最终记录；认证接受输出 `authenticated_external_review/accepted`，认证拒绝输出 `rejected` 并返回退出码 1，无记录继续保持 `requires_external_review` |
| F-07 | trust store、启动记录和验收记录均位于仓库外并在发布结果前重读；签名 envelope、文件和总输入沿用既有大小、类型与时间预算 |

## 阈值算法

检测、误报、修复和检查完成率在全部 Qualitygate group 上合并分子、分母和 unknown；unknown 非零或
零分母保持 `unknown`。复核比例按已复核诊断/全部 distinct 诊断计算。每个 Qualitygate comparison
分别检查相同 input inventory、审查时间减少、full P95 比例和每个合格交付成本比，任一 pair 的
false 使对应检查 `not_met`，缺值使其 `unknown`。已知失败不会被另一个未知 pair 隐藏。

评估记录包含算法 ID `qualitygate-pilot-thresholds-v1` 和完整 groups/comparisons 摘要，避免只签署
一个脱离口径的布尔值。协议中的非结构化金额上限、归档权威、任务真值和因果解释仍由 reviewer
结合保留证据判断；CLI 不从自由文本金额推导数值结论。

## 所有权与数据流

`domain::pilot::acceptance` 负责纯阈值判定、manifest/启动授权绑定和验收主题；
`adapters::pilot_acceptance` 只在 DSSE/Ed25519 认证后解释 reviewer 决策。
`application::pilot` 编排仓库外输入、报告加载、两次签名复验和发布前不变检查；`interfaces`
只解析 `acceptance-subject`、`--acceptance` 和渲染结果。依赖保持无环。

## 接受边界

认证结果证明配置的 reviewer 对精确证据作出了接受或拒绝决定。它不制造真实观察、不证明线上无缺陷，
也不把相关性变成因果收益。具体 8 个生产任务、正式 owner/reviewer、外部归档和七天数据仍需团队提供；
在这些输入不存在时，仓库测试只能证明验收机制。

## 验证

领域单元测试覆盖 9 项全部通过、已知失败、未知值和失败/未知并存。adapter 单元测试覆盖接受、拒绝、
阈值不足、提前签发、撤销和主题漂移。`pilot_acceptance` 集成测试构造完整双模型×双流程的两次尝试，
覆盖主题生成、独立签名、接受、拒绝退出码和 observation 变化导致的旧签名失效。
