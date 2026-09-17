# 阶段 L：模型采集记录与未知实际版本

> 日期：2026-09-17；工程实现；正式 8 项任务、7 天试点仍未启动。

## 使用方式

新试点从 [v7 清单模板](../templates/pilot/observation-v7.json)开始。封存前按既定矩阵
填写 `cohort.requested_model`、`reasoning_effort`、`agent_version` 和
`harness_digest`。封存后的每个 `observation` 增加 `model_evidence`；其中
`artifact` 指向外部归档内的一份 JSON 文件，`capture` 与该文件反序列化内容完全相同。

```json
{
  "artifact": {"path":"model/run-01.json","digest":"sha256:…","bytes":234},
  "capture": {
    "assignment_id":"run-01", "captured_at":1780000000,
    "agent_version":"codex-cli version", "harness_digest":"sha256:…",
    "requested_model":"requested model", "reasoning_effort":"medium",
    "actual_model":null, "status":"unknown",
    "unknown_reason":"Provider response did not expose a routed model identifier"
  }
}
```

示例时间、标识和摘要只是字段说明，不可直接用于正式试点。`reported` 状态要求
具体 `actual_model`，且 `unknown_reason=null`；`unknown` 状态要求
`actual_model=null` 和非空原因。`cohort.actual_model` 应与采集记录一致。
采集时间须在协议窗口内且不晚于 `observation.observed_at`。文件至多 64 KiB，
所有模型记录至多 8 MiB；CLI 拒绝越界、符号链接、字节漂移和清单/文件内容不一致。

`pilot seal` 只绑定观察前请求配置；`pilot summarize` 和最终验收主题会重新读取
模型文件。`model_audit` 逐运行单元列出请求、采集、实际或未知状态。已观察运行缺
记录是证据不完整；实际模型明确未知是可记录的结果，v7 不再仅因此阻断协议。
比较范围仍是**请求模型配置**，不能据此宣称服务商实际路由版本的因果差异。

## 可信边界

SHA-256 及封存可证明归档记录在比对时与声明一致，无法证明归档内容来自服务商，
也无法独立证明本地采集时间。正式试点需由独立 reviewer 将 `reported` 声称、
`unknown` 原因、Agent/harness 版本和启动日志对照外部原始记录。旧 v1–v6
清单维持原有封存 JSON 与验收语义。

## 需求到测试

| 要求 | 证据 |
|---|---|
| L-01/L-03 | `domain::pilot::tests::v7`：请求身份、未知原因、时间、已报告模型与 v6 拒绝新字段 |
| L-02 | `pilot_seal::cli_v7_binds_model_capture_and_preserves_explicit_unknown_identity`：真实文件、漂移、清单/文件不一致、越界和符号链接 |
| L-04 | 同一 CLI 用例和领域汇总：`model_audit`、缺记录与明确未知的不同结果 |
| 兼容性 | 上一安装版与新版对同一 v6 清单的封存 JSON 字节比较；全部既有 pilot 回归 |

## 工程验证

2026-09-17 本地 Linux 验证通过 `cargo fmt --all -- --check`、全目标/特性编译、
Clippy `-D warnings` 和全目标/特性测试：441 项通过、0 失败，12 项按既有外部
生产者、凭据或收费模型条件忽略。LLVM Rust 行覆盖率为 95.82%（23,068 行中
965 行未覆盖），超过 90% 门禁。架构报告包含 162 个源码摘要、1,520 条
文件/行依赖、0 违规；文档门禁、CodeSpec/Knowledge map、Bazel 11 个测试目标
及根 CLI 构建均通过。debug、release、staged Skill 和安装版完整 selfcheck 均为
338/338、complete/pass。

上一安装版与新版对同一份 8 项、32 单元 v6 计划封存的完整 JSON 字节相同，
plan digest 为 `sha256:082bfdc43e36f12719892ad89a99dad4de65b9dff098046a68fe84a58fb65770`。
本次 release 二进制摘要为
`sha256:cc8aad5865c3e4dbfc4df497ed68a73f8756a74d36b9504a7524dc1bf768c38c`。
合成 v7 八任务计划成功封存；安装版对一个实际模型明确未知的合成观察显示
`model_audit.status=unknown`，未因该原因增加协议限制。其余 31 个单元未观察，
因此整体仍为 incomplete，退出码 2。机器可读输出和日志位于
`target/pilot-phase-l-verification/`。

按此前用户要求，本地开发 Skill 已更新到 `~/.codex/skills/qualitygate-cli`；
上一安装版备份于
`~/.codex/skill-backups/qualitygate-cli-20260917T021842Z`。
这是 dirty source 的本地开发安装，不是公开发布版。合成采集记录只验证工程契约，
不属于正式收益样本；没有重新运行真实模型、取得正式签名或开始七天观察。
Miri、ASan 和外部 producer 仍属于各自独立门禁。
