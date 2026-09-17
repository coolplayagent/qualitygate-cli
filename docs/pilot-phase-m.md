# 阶段 M：逐尝试模型采集与路由漂移

> 日期：2026-09-17；工程实现；正式 8 项任务、7 天试点仍未启动。

## 使用方式

新试点从 [v8 清单模板](../templates/pilot/observation-v8.json)开始，在观察前照原
协议封存请求模型、Agent/harness 身份、任务矩阵、预算与顺序。封存时
`cohort.actual_model=null`。每次尝试（包括失败、超时、放弃）都在
`attempt.model_evidence` 放入工件和对应采集内容，运行单元级
`observation.model_evidence` 在 v8 禁用：

```json
{
  "artifact": {"path":"model/run-01-attempt-2.json","digest":"sha256:…","bytes":252},
  "capture": {
    "assignment_id":"run-01", "attempt_number":2,
    "captured_at":1780000000, "agent_version":"codex-cli version",
    "harness_digest":"sha256:…", "requested_model":"requested model",
    "reasoning_effort":"medium", "actual_model":null,
    "status":"unknown", "unknown_reason":"Provider did not expose routing"
  }
}
```

将 `capture` 对象单独保存为外部归档 JSON，`artifact` 填该文件相对路径、
真实字节数和 SHA-256。示例不可直接作试点证据。CLI 对每份文件核对普通文件、
受限路径、字节和严格 JSON 内容，单份上限 64 KiB，总量上限 8 MiB；每次
汇总和最终验收前重读。缺任一次记录保持 `complete=false`。实际模型明确
未知时保留原因；若全部尝试报告同一实际模型，`cohort.actual_model` 必须为
该值，否则必须为 null。同一单元内两种不同的**已报告**实际模型使
`protocol_ready=false`，不可签署最终接受。

`model_audit` 逐尝试列出尝试状态、采集状态、请求与实际模型、未知原因、
采集时间、工件摘要，并列出缺失尝试、未观察单元和路由漂移单元。
比较仍按请求配置进行。归档摘要不能证明提供者实际路由、Agent 日志或
本地时钟，独立 reviewer 必须对照原始记录。

## 需求到测试

| 要求 | 证据 |
|---|---|
| M-01/M-03 | `domain::pilot::tests::v8`：封存、失败/超时、尝试号、请求身份、时间、实际模型和旧版本字段约束 |
| M-02 | `pilot_seal::v8::cli_v8_verifies_each_attempt_model_file_and_exposes_route_drift`：实际文件、缺失、漂移、越界、路径与符号链接 |
| M-04 | 同两组测试：`model_audit` 逐尝试输出、明确未知与不同已报告模型的协议偏离 |
| 兼容性 | 旧版与新版对相同 v7 输入的完整封存 JSON 字节比较；既有 pilot 回归和 Skill v7/v8 模板一致性 |

## 工程验证

2026-09-17 本地 Linux 验证通过 `cargo fmt --all -- --check`、
`cargo check --all-targets --all-features`、Clippy `-D warnings` 和
`cargo test --all-targets --all-features`：447 项通过、0 失败，12 项按既有
外部生产者、凭据或收费模型条件忽略。全量测试首次并发运行时，既有
`harness_timeout_keeps_the_attempt_and_unfinished_status` 用例遇到一次
2 秒边界抖动；隔离重跑及完整重跑通过，没有修改用例或门禁。
LLVM Rust 行覆盖率为 95.84%（23,247 行中 966 行未覆盖），超过 90%。
架构报告有 162 个源码摘要、1,520 条文件/行依赖、0 违规；文档门禁、
CodeSpec/Knowledge map、Bazel 11 个测试目标及根 CLI 构建均通过。

上一安装版与新版对相同 32 单元 v7 计划的封存 JSON 逐字节一致，
SHA-256 为 `cc6a1b76f8d4cefc9cb02e1e22ab4c769ab847fb5383bbf8e4d19aac9ca1153f`。
对旧 v7 合成观察的汇总，除调用时生成的 `evaluated_at` 外字段一致。
暂存和安装的 Skill 完整 selfcheck 均为 338/338、complete/pass。
release 二进制 SHA-256 为
`f9472e8d7732e895783fc828f6cdb2b600c94debee4868f5b82359a62a63ac54`。
安装版成功封存合成 v8 32 单元计划；合成观察中一次失败尝试有明确未知模型，
一次超时尝试缺模型文件。`model_audit` 分别显示 `unknown` 与 `missing`，
并保留封存的请求模型和尝试状态。其余 31 个单元未观察，整体为 incomplete，
CLI 退出码 2。
日志与机器可读证据位于 `target/pilot-phase-m-verification/`。

此前用户要求的本地开发 Skill 已更新到 `~/.codex/skills/qualitygate-cli`；
旧安装版备份于 `~/.codex/skill-backups/qualitygate-cli-20260917T0355Z-phase-m`。
这是本地开发安装，不是公开发布。合成记录只验证工程契约，不能代替
生产来源、真实模型日志、正式签名、7 天观察或人工收益测量。Miri、ASan
和外部 producer 仍属于各自独立门禁。
