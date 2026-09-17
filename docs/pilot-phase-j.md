# 阶段 J：交替执行顺序封存与审计

> 日期：2026-09-16；工程能力已实现；真实 8 项任务/7 天试点仍未启动。

## 1. 观察前准备

新试点从 [v5 清单模板](../templates/pilot/observation-v5.json)填写。仍使用已确认的
八项独立任务（缺陷修复、重构各四项）、两种 Codex 模型组、两种流程、七天和既定预算。
`run_order` 是 32 个 assignment ID 的完整有序列表，观察前随协议一起封存。
列表相邻项的流程必须在 `existing_tools` 与 `qualitygate` 之间交替。
对每个任务类型和请求模型，四项任务中两项须既有流程先行、另两项须 qualitygate 先行。
这样顺序效应不会全部落在某一流程。若更改任务或运行次序，须在观察前重新封存并重新授权。

每个观察的 `start_sequence` 从 1 开始，表示该 assignment **实际启动**的位置。
例如计划列表首项的序号应为 1；结束时间 `observed_at` 不能代替启动次序。
重复、零或超出计划范围的序号拒绝读取；缺失项在 `schedule_audit` 标为 incomplete，
位置不符标为 deviated。两种状态均使 `protocol_ready=false`，不能生成有效的最终接受主题。

## 2. 证据边界

序号和任务来源、复核时间一样是调用方声明。CLI 只能检查排列、分层平衡、封存漂移和
声明的一致性；外部 harness 应保存不可变启动日志，human reviewer 对照运行单元 ID、
实际开始事件及其先后。`plan_seal` 绑定计划顺序；最终 manifest 摘要绑定实际序号。
v1–v4 清单继续原有 JSON/摘要语义，不含 `run_order` 或 `start_sequence`。

## 3. 需求到测试

| 要求 | 证据 |
|---|---|
| J-01/J-03/J-05 | `domain::pilot::tests::v5::v5_seals_alternating_order_and_audits_observed_start_sequence` |
| J-02 | `domain::pilot::tests::v5::v5_rejects_unbalanced_workflow_first_counts_within_a_stratum`、`v5_seals_eight_inputs_with_two_first_positions_per_stratum` |
| J-01/J-03 | `pilot_seal::cli_v5_seals_schedule_and_blocks_observed_order_deviation` |
| J-05 | 上一安装版与新版对同一 v4 清单的完整封存 JSON/摘要比较；全部既有 pilot 测试 |
| J-04 | 本文外部启动日志与人工复核边界；正式执行日志尚待记录 |

## 4. 工程质量结果

2026-09-16 的本地 Linux 验证通过 `cargo fmt --all -- --check`、全目标/特性编译、
Clippy `-D warnings` 和全目标/特性测试：434 项通过、0 失败，12 项依既有外部
生产者、凭据或收费模型合同忽略。LLVM Rust 行覆盖率为 95.78%（22,698 行，
957 行未覆盖），超过 90% 门禁。debug、release、staged Skill 和安装版 full
selfcheck 均为 338/338、complete/pass。

架构报告覆盖 161 个源码摘要、1,505 条文件/行依赖、0 违规；domain、config、
adapters、application、interfaces 五个 Bazel 单元目标及根 CLI 构建通过。
CodeSpec 与 Knowledge map 有效。日志和机器可读输出保存在
`target/pilot-phase-j-verification/`。

上一安装版与本次 release 对同一份 8 项、32 单元 v4 计划封存的 JSON 字节完全相同，
plan digest 为 `sha256:8bf4874e8e238fdf031e4bd411efa433a47e162661894242d52b377001e8077a`。
合成的 v5 八任务计划完成 32 单元封存，摘要为
`sha256:6554812eaebc5d438c8d6a96103e635fb901bd6d2b9d57fb9825b4b8095c48b6`。
32 个正确启动序号给出 `schedule_audit.status=matched`；交换前两个序号后给出
`deviated` 和 `protocol_ready=false`。两份观察均没有真实报告，故完整性与正式接受
仍为未完成；`compatibility.json` 保存二进制和输出摘要。

按此前用户要求，本地开发 Skill 已更新到 `~/.codex/skills/qualitygate-cli`，上一安装版
备份于 `~/.codex/skill-backups/qualitygate-cli-20260916T090229Z`。安装版 full selfcheck
为 338/338；release 二进制摘要为
`sha256:0908ce4406e022ddb29cd294eec6dc72cb24e2aaf02a4f070b4bd6c46a668f38`。
安装版 v5 偏离审计烟测通过；`installation.json` 标记这是 dirty source 的本地开发安装，
不是公开发布版。

合成文件、任务、序号和身份只验证契约，不是正式试点样本。没有重新运行真实模型、
采集可信启动日志、取得正式签名或开始七天观察；Miri、ASan 与外部 producer
仍属于各自独立门禁。
