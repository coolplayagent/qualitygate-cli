# 阶段 N：逐尝试执行记录与时间顺序

> 日期：2026-09-17；工程实现；正式 8 项任务、7 天试点尚未启动。

## 使用方式

新试点从 [v9 清单模板](../templates/pilot/observation-v9.json)开始，观察前封存
任务、预算、harness 身份和 `run_order`。对每次尝试，包括失败、超时、
放弃和不完整尝试，在 `attempt.execution_evidence` 记录工件和内容：

```json
{
  "artifact": {"path":"execution/run-01-attempt-2.json","digest":"sha256:…","bytes":280},
  "capture": {
    "assignment_id":"run-01", "attempt_number":2,
    "started_at_ms":1780000000000, "ended_at_ms":1780000001000,
    "harness_digest":"sha256:…", "status":"timed_out",
    "snapshot_digest":"sha256:…", "report_digest":null
  }
}
```

将 `capture` 对象保存为外部归档严格 JSON，按真实内容填写相对路径、字节数
和 SHA-256。示例中的占位符不可作为试点证据。CLI 核对 assignment、尝试号、
封存 harness、状态、快照和报告摘要；结束减开始必须等于清单 `elapsed_ms`。
时间须在封存的观察窗口内且不晚于 `observed_at`，同一运行单元中已归档
尝试不得重叠。`execution_audit` 显示每次尝试及缺失记录，并按
`start_sequence` 检查已归档的首次启动时间。相同或逆序为协议偏离。

每份文件最多 64 KiB，总量最多 8 MiB；路径不得越界、跟随符号链接或
指向非普通文件。CLI 在封存、启动授权主题、汇总和最终验收主题前重读。
缺记录保持 `complete=false`；字段矛盾或文件漂移拒绝输出。v1–v8 记录
保持原有封存和汇总语义。

归档摘要只证明字节与清单一致，不能证明 harness 真正执行过、时钟可靠
或状态属实。独立 reviewer 仍须对照原始 Agent、进程与时钟日志。

## 需求到测试

| 要求 | 证据 |
|---|---|
| N-01/N-02 | `domain::pilot::tests::v9`：失败/超时、缺记录、时间差、状态/身份/报告失配、重叠、旧版字段拒绝 |
| N-03 | `pilot_seal::v9::cli_v9_rereads_receipts_and_rejects_drift_and_unsafe_files`：实际文件、漂移、越界、链接和大小限制 |
| N-04 | `pilot_seal::v9::cli_v9_detects_start_sequence_conflict_with_archived_clock`：启动时间逆序阻止协议接受 |
| 兼容性 | 既有 v1–v8 pilot 回归、v7 封存 JSON 字节比较、Skill 模板一致性 |

## 工程验证

2026-09-17 本地 Linux 验证通过 `cargo fmt --all -- --check`、
`cargo check --all-targets --all-features`、Clippy `-D warnings` 和
`cargo test --all-targets --all-features`：452 项通过、0 失败，12 项按既有
外部生产者、凭据或收费模型条件忽略。LLVM Rust 行覆盖率 95.83%
（23,436 行中 978 行未覆盖），超过 90%。架构报告有 163 个源码摘要、
1,527 条文件/行依赖、0 违规；文档门禁、CodeSpec/Knowledge map 验证
与 Bazel 11 个测试目标均通过。

`cargo package --locked --allow-dirty` 验证通过；`--allow-dirty` 仅用于
包含本次未提交改动的本地工作树，发布工作流仍要求干净检出。包内包含
v9 设计、模板和审计源码。对同一份旧 v8 封存输入，安装前二进制与新版
生成的授权主题逐字节一致（SHA-256
`d6fe619de78336d82a2293f5bfe62540032e61600fdeb0517bb1fe3fecd7b70`）；
汇总除调用时生成的 `evaluated_at` 外相同。

本地开发 Skill 已更新到 `~/.codex/skills/qualitygate-cli`，旧版备份于
`~/.codex/skill-backups/qualitygate-cli-20260917T0514Z-phase-n`。
Linux x64 release 二进制 SHA-256 为
`778dcebd6f5a322234f9302f7b04b13f2ba2211dc7face81bda0bc1668355135`。
暂存和安装版的完整 selfcheck 均为 338/338、complete/pass；安装版在
独立空目录成功列出内置规则。本仓库的 `qualitygate/rules` 目录无 YAML
定义，直接在项目根执行 `rules list` 如实返回 incomplete，不作为工具通过
证据。日志与机器可读输出位于 `target/pilot-phase-n-verification/`。

合成记录只验证工程契约。Miri、ASan 与外部 producer 属于独立门禁；
正式试点仍需真实任务来源、原始模型和时钟日志、受信签名、7 天观察及
独立人工收益复核。
