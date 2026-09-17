# 阶段 F：由独立 reviewer 签署最终试点结论

> 日期：2026-09-16；工程能力已实现；真实生产试点与最终结论尚不存在。

## 1. 生成验收主题

完成七天观察并保留全部 report 后，先重验阶段 E 的启动授权，再生成最终主题：

```bash
qualitygate pilot acceptance-subject \
  --input observations.json \
  --trust-store /external/pilot/trust.json \
  --authorization /external/pilot/start.dsse.json \
  --format json > acceptance-subject.json
```

命令要求 `complete=true`、`protocol_ready=true`、观察窗口已结束、实际模型已记录且报告 bytes 与摘要一致。
subject 绑定完整 manifest、plan seal、启动授权 signer，以及 9 项阈值检查和 groups/comparisons 摘要。

## 2. 阈值结果

`subject.assessment.outcome` 只有三种状态：

- `met`：所有必需指标都已知且通过；
- `not_met`：至少一个指标明确失败；
- `unknown`：没有明确失败，但仍有零分母、缺失复核分母、缺成本或不可比较对照。

检查包括全体 Qualitygate 组的检测率、误报率、修复率、必需检查完成率和复核比例，以及所有
Qualitygate/既有工具 pair 的 inventory 一致性、审查时间减少、full P95 比例和成本比。
CLI 保留分子、分母、unknown 和 pair 的通过/失败/未知数量。

## 3. reviewer 签名

协议中的独立 reviewer 用 `pilot-trial-acceptance` scope 的 human Ed25519 key 签署：

```json
{
  "schema_version": 1,
  "record_id": "qualitygate-pilot-review-2026-09-23",
  "subject": {},
  "reviewer": {"id": "pilot-reviewer", "kind": "human"},
  "decision": "accepted",
  "reason": "All retained thresholds and external evidence were reviewed",
  "issued_at": 1790121700,
  "expires_at": 1790726500
}
```

payload type 为 `application/vnd.qualitygate.pilot-trial-acceptance.v1+json`。`accepted` 仅在 assessment
为 `met` 时合法；`not_met` 或 `unknown` 可签 `rejected`，不能签接受。签发时间不能早于观察结束。
owner 和 reviewer 必须不同，trust-store key ID 必须等于协议 reviewer。
schema v2+ 另按[阶段 G](pilot-phase-g.md)加入结构化总预算第十项；本节九项口径
保留给已封存的 schema v1 清单。

## 4. 发布认证汇总

```bash
qualitygate pilot summarize \
  --input observations.json \
  --trust-store /external/pilot/trust.json \
  --authorization /external/pilot/start.dsse.json \
  --acceptance /external/pilot/review.dsse.json \
  --format json
```

认证接受输出 `authority=authenticated_external_review`、`trial_acceptance=accepted`。认证拒绝输出
`trial_acceptance=rejected` 并返回 1。未提供最终记录时，完整汇总仍成功生成，但保持
`requires_external_review`；缺报告或验证失败返回 2。每次调用都会重新检查两个 envelope、trust store、
有效期和撤销状态，且在结果发布前重读外部文件。

## 5. 证据边界

数字签名认证 reviewer 对精确 manifest 和指标结果的决定。非结构化金额上限、归档服务权威、任务真值、
真实业务代表性和因果收益仍需要 reviewer 结合外部证据判断。接受不代表线上无缺陷或授权自动推广规则。

当前仓库没有真实 8 项生产任务和七天 observation。本阶段集成测试使用临时仓库、合成 report 和固定测试 key，
所以没有生成正式试点结论。

## 6. 需求到测试

| 需求 | 验证 |
|---|---|
| F-01/F-02 | `domain::pilot::acceptance::all_required_thresholds_must_be_known_and_met` |
| F-02 | `domain::pilot::acceptance::known_comparison_failure_is_not_hidden_by_an_unknown_pair` |
| F-04/F-05 | `adapters::pilot_acceptance::independent_reviewer_can_accept_only_met_exact_evidence` |
| F-04 | `adapters::pilot_acceptance::acceptance_rejects_early_foreign_and_revoked_review` |
| F-01/F-03/F-06 | `pilot_acceptance::cli_binds_thresholds_and_reports_authenticated_reviewer_decisions` |
| F-03/F-07 | `pilot_acceptance::cli_rejects_an_acceptance_after_observation_evidence_changes` |

## 7. 工程质量结果

2026-09-16 的本地 Linux 验证通过格式、全部目标/特性编译、Clippy warnings denied 和
全部目标/特性测试：423 项通过，12 项需要外部生产者、真实凭据或收费模型的测试按合同忽略。
LLVM 覆盖率为 95.76% Rust lines（22,310 行，946 行未覆盖），超过 90% 门禁。

full selfcheck 的 338 个 fixture 全部与 golden 一致。架构报告覆盖 159 个源摘要和 1,501 条
文件/行依赖，0 违规。domain、config、adapters、application、interfaces 五个 Bazel 单元目标
通过，根 CLI Bazel target 构建通过。CodeSpec 与 Knowledge map 均有效。日志和机器可读报告
保存在 `target/pilot-phase-f-verification/`。

此前用户要求的本地开发 Skill 已同步到 `~/.codex/skills/qualitygate-cli`。staged 和安装后
selfcheck 均为 338/338，安装后的 `acceptance-subject` 命令及 `summarize --acceptance` 可用，
release binary 摘要为
`sha256:dbe84baa5cec698d3b97a0152f0b0879228a2c13c7f14902b3ea8edb7b61ada0`；上一安装备份在
`~/.codex/skill-backups/qualitygate-cli-20260916T071233Z`。这是带 dirty source 标记的本地
development install。

工程验证没有运行新的模型调用、没有生成正式 reviewer 签名，也没有开始或替代真实 8 项任务/
七天观察；Miri、ASan 和外部 producer 继续属于各自独立门禁。
