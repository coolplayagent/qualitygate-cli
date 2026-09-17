# 阶段 K：修复预算与无进展停止审计

> 日期：2026-09-16；工程能力已实现；真实 8 项任务/7 天试点仍未启动。

## 1. 观察前准备

新试点从 [v6 清单模板](../templates/pilot/observation-v6.json)填写。每个 assignment
除阶段 J 的交替 `run_order` 外，还必须有修复前的 `initial_report`：

```json
{"path":"reports/run-01-initial.json","digest":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","bytes":1234}
```

示例摘要和长度不能用于正式封存。报告须为同一任务、受信策略、环境、工具、初始快照和
必需检查的完整 `full` 门禁。报告文件位于清单所在的外部归档内，使用受限相对路径；
每份至多 16 MiB，初始报告与全部尝试报告共享 64 MiB 读取预算。封存、启动授权主题、
汇总和最终验收主题均复核文件；`initial_report` 的路径、摘要和长度被 plan seal 绑定。

协议新增 `no_progress_limit`，本试点按已确认规则填 `2`；`max_attempts=3`、
`max_seconds=1800` 均沿用阶段 A 的 3 次／30 分钟参数。三项值在观察前封存。

## 2. 进展与停止判定

与[阶段 B harness](agent-loop.md)一致，进展要求下一份完整、绑定的 full 报告中，
错误诊断 `check_id:fingerprint` 与 gate blocker 的集合是上一份报告的**真子集**。
审计先从检查结果独立重算 blocker，拒绝报告中人为减少的 blocker 列表。
只改源码、换一个错误或保持相同错误集都不算进展。失败、超时或放弃且无复验报告的
尝试也不算可验证进展，但仍保留为失败尝试。已声明完成却缺报告、报告不完整或身份不符
则是未完成证据。

`attempt_audit` 按运行单元列出初始报告摘要、每次尝试的报告摘要、前后债务数、进展判定、
累计耗时、预算状态和停止位置。超 3 次、累计超过 30 分钟，或连续两次无进展后继续
尝试，均为协议偏离，`protocol_ready=false`。缺少有效证据时 `complete=false`。
所有尝试及成本仍留在汇总中，不因超限而删除。

报告摘要和严格子集运算只能证明归档数据内部一致。尝试耗时、Agent 失败原因及报告
是否来自真实运行，仍须由外部 harness 日志和独立 reviewer 对照。旧 v1–v5 清单维持
原有摘要与验收语义，不带入此审计。

## 3. 需求到测试

| 要求 | 证据 |
|---|---|
| K-01/K-02 | `domain::pilot::tests::v6::v6_seal_binds_verified_initial_reports_and_progress_limit` |
| K-03/K-04 | `v6_audit_stops_after_two_unverified_reductions_and_retains_later_attempts`、`v6_audit_resets_only_on_strict_debt_reduction_and_checks_both_budgets` |
| K-02/K-04 | `pilot_seal::cli_v6_rechecks_initial_reports_and_audits_stopping_after_two_no_progress_attempts` |
| K-01/K-05 | 上一安装版与新版对同一 v5 清单的完整封存 JSON/摘要比较；全部既有 pilot 测试 |
| K-05 | 本文外部日志与真实性复核边界；正式试点证据尚待记录 |

## 4. 工程质量结果

2026-09-16 的本地 Linux 验证通过 `cargo fmt --all -- --check`、全目标/特性编译、
Clippy `-D warnings` 和全目标/特性测试：438 项通过、0 失败，12 项依既有外部
生产者、凭据或收费模型合同忽略。LLVM Rust 行覆盖率为 95.82%（22,946 行，
960 行未覆盖），超过 90% 门禁。debug、release、staged Skill 和安装版 full
selfcheck 均为 338/338、complete/pass。

架构报告覆盖 162 个源码摘要、1,513 条文件/行依赖、0 违规；domain、config、
adapters、application、interfaces 五个 Bazel 单元目标及根 CLI 构建通过。
CodeSpec 与 Knowledge map 有效。日志和机器可读输出保存在
`target/pilot-phase-k-verification/`。

上一安装版与本次 release 对同一份 8 项、32 单元 v5 计划封存的 JSON 字节完全相同，
plan digest 为 `sha256:d7661f47491dbfeb78b41535370a58f4a06e0db90c010e98e1d454baaece07bc`。
合成的 v6 八任务计划绑定 32 份不同初始报告并成功封存，摘要为
`sha256:dd2e730653150859f0fc6bd029006bd2f019fc2d38fabe9a5664e6f5cda01a3c`。
32 份初始报告均在汇总中得到验证；为其中一个运行单元添加三次无报告失败尝试后，
审计在第 2 次给出停止点，第 3 次标记 `continued_after_no_progress`，
`protocol_ready=false`。其余运行单元无观察，故正式完整性仍为未完成。
`compatibility.json` 保存二进制和输出摘要。

按此前用户要求，本地开发 Skill 已更新到 `~/.codex/skills/qualitygate-cli`，上一安装版
备份于 `~/.codex/skill-backups/qualitygate-cli-20260916T093633Z`。安装版 full selfcheck
为 338/338；release 二进制摘要为
`sha256:32a630a6ca08f6d75a1194704d641cd2ee63c40d6d20f4f00d0994623965ce64`。
安装版 v6 封存及停止偏离烟测通过；`installation.json` 标记这是 dirty source 的本地
开发安装，不是公开发布版。

合成报告、任务、耗时和身份只验证契约，不是正式收益样本。没有重新运行真实模型、
取得正式签名或开始七天观察；Miri、ASan 与外部 producer 仍属于各自独立门禁。
