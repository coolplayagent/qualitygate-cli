# 阶段 G：结构化总预算验收

> 日期：2026-09-16；工程能力已实现；真实 8 项任务/7 天试点仍未启动。

## 1. 准备新计划

新试点从 [v4 清单模板](../templates/pilot/observation-v4.json)开始；v2/v3 模板保留给已有计划。
保留已确认的 8 项任务、7 天、
Codex 中/低模型组、每单元 3 次/30 分钟及原阈值，在观察前填写具体任务、独立 owner/reviewer、
持久归档与预算：

```json
"budget": {
  "currency": "USD",
  "priced_at": 1790000000,
  "source": "Reviewed model, infrastructure and human rate card reference",
  "max_total_micros": 100000000,
  "human_hourly_micros": 30000000
}
```

以上数字仅为格式示例，不是本试点批准的金额。`max_total_micros` 覆盖 32 个运行单元的全部
模型、基础设施与人工审查费用；微币单位是相应币种的百万分之一。`priced_at` 是价格表版本时间，
不是独立可信时间戳。v2+ 将旧 `monetary_cap` 留为 null，避免两个上限冲突。费用和人工时薪来源
需由团队复核，CLI 不获取真实账单或人事费率。

按[阶段 D](pilot-phase-d.md)执行 `pilot seal`，再按[阶段 E](pilot-phase-e.md)取得外部 owner
启动授权。预算属于计划字段；封存后改动任何值都会让旧摘要和授权失配。

## 2. 汇总与最终签名

每次尝试的 `cost` 使用同一 `currency` 和 `priced_at`，并分别填写 `model_micros`、
`infrastructure_micros`；每条 observation 填写 `review_active_ms`。失败、超时和放弃的尝试
同样计费。人工费用按每条 observation 的活跃毫秒与封存时薪向上取整。

`pilot summarize` 的 `budget` 保存已知模型、基础设施、人工与合计金额、缺口数和状态。
缺运行、零尝试、缺成本分量、缺人工分钟或货币/价格时间不一致时是 `unknown`；若已知部分
已经超过上限，则是 `not_met`。与基线的每合格交付成本比仍单独呈现，不能抵销全试点超额。

按[阶段 F](pilot-phase-f.md)调用 `pilot acceptance-subject`。v2+ 使用
`qualitygate-pilot-thresholds-v2` 和十项检查，第十项 `total_cost_micros` 为上限比较。
预算为 `not_met` 或 `unknown` 时，独立 reviewer 只能签 `rejected`；`accepted` 会被拒绝。
旧 schema v1 封存记录继续按九项检查解释，其自由文本上限仍需人工判断。

## 3. 需求到测试

| 要求 | 证据 |
|---|---|
| G-01/G-02 | `domain::pilot::tests::v2_seal_requires_a_bounded_structured_budget_and_binds_it` |
| G-03/G-04 | `domain::pilot::tests::structured_budget_counts_failed_attempts_and_rejects_unknown_or_overspend` |
| G-05 | `domain::pilot::acceptance::tests::structured_total_budget_is_a_tenth_required_check` |
| G-05/G-06 | `pilot_acceptance::cli_binds_thresholds_and_reports_authenticated_reviewer_decisions` |
| v1 兼容 | 阶段 F 的九项领域测试及既有签名 adapter 测试 |

## 4. 工程质量结果

2026-09-16 的本地 Linux 验证通过格式、全部目标/特性编译、Clippy warnings denied 和
全部目标/特性测试：426 项通过，12 项需要外部生产者、真实凭据或收费模型的测试按合同忽略。
LLVM 覆盖率为 95.75% Rust lines（22,459 行，955 行未覆盖），超过 90% 门禁。

full selfcheck 的 338 个 fixture 全部与 golden 一致。架构报告覆盖 160 个源摘要和 1,501 条
文件/行依赖，0 违规。domain、config、adapters、application、interfaces 五个 Bazel 单元目标
通过，根 CLI Bazel target 构建通过。CodeSpec 与 Knowledge map 均有效。日志和机器可读报告
保存在 `target/pilot-phase-g-verification/`。

同一旧版 v1 计划分别交给上一安装版与本次二进制封存，完整 JSON 与
`sha256:38181befef28221da50fe194c15361bfb02228354d6d9b82d57607642e252da2`
摘要完全相同；对照文件为 `v1-compatibility.json`。新 v2 计划在 staged 和已安装 release 上
均成功封存。首次 staged 烟测使用了已过启动时间的输入，正确返回未完成；原始输出保留为
`staged-v2-expired-window.json`，随后使用新的未来窗口复测通过。

此前用户要求的本地开发 Skill 已更新到 `~/.codex/skills/qualitygate-cli`。staged 与安装后
selfcheck 均为 338/338，release binary 摘要为
`sha256:de5753b0134c6abe8b2c22359da06ffa8d4526fcf6cccc090606211772c53ea1`；
上一安装备份在 `~/.codex/skill-backups/qualitygate-cli-20260916T074344Z`。这是带 dirty
source 标记的本地 development install。

这些证据只覆盖工程实现。没有重新调用模型、获取正式账单或人工费率，也没有开始真实 8 项任务/
七天观察；Miri、ASan 和外部 producer 继续属于各自独立门禁。
