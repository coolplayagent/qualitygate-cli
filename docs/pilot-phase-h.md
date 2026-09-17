# 阶段 H：独立任务分层与重复样本拦截

> 日期：2026-09-16；工程能力已实现；真实 8 项任务/7 天试点仍未启动。

## 1. 准备与封存

阶段 H 从 [v3 清单模板](../templates/pilot/observation-v3.json)填写；阶段 I 起的新试点使用
[v4 清单模板](../templates/pilot/observation-v4.json)并补来源工件。已确认的 8 项任务分成
4 个缺陷修复与 4 个重构，`protocol.task_mix` 将配额机器可读地固定。每项任务只算一个
`input_id`，但仍须分配给 Codex 中、低模型组及 existing-tools、qualitygate 两种流程，共
32 个运行单元。每个输入的四个运行单元共享同一个任务 ID、类型、契约摘要和预期问题。

`pilot seal` 要求配额总和等于 `task_count`，实际去重输入的类型计数等于配额，且不同输入的
`task_id` 与 `task_digest` 不重复。只换 `input_id` 再复制同一任务，或把一种任务类型误填成
另一种，均无法封存。owner、reviewer、七天窗口、结构化预算及其它阶段 D–G 条件照常校验。

先由团队对照真实 issue/commit、批准的任务契约和抽样记录检查八项任务确实独立，再把首次
封存输出及摘要存入声明的外部归档。CLI 无法从不同 ID 或摘要证明两个任务在语义上独立。

## 2. 汇总、兼容性和验收边界

v3 的 `pilot summarize` 增加 `sampling_audit`：声明/实测的各类型任务数、不同输入数及每个
输入的 ID、任务 ID、类型和契约摘要。全部字段已由 manifest digest 绑定，观察期间修改配额
或任务身份会使原 seal、启动授权与最终验收主题失配。

v3 沿用 v2 的结构化总预算、十项阈值和 `qualitygate-pilot-thresholds-v2` 评估语义。
v1/v2 已封存记录保持原序列化、摘要与验收解释。外部 owner 签名、独立 reviewer 决策和真实
七天观察仍按[阶段 E](pilot-phase-e.md)、[阶段 F](pilot-phase-f.md)执行。

## 3. 需求到测试

| 要求 | 证据 |
|---|---|
| H-01–H-04 | `domain::pilot::tests::v3_seal_enforces_declared_strata_and_independent_task_identities` |
| H-01–H-04 | `pilot_seal::cli_v3_seal_audits_task_mix_and_rejects_repeated_task_contracts` |
| H-05 | 相同 v2 计划的旧安装版/新版封存 JSON 与摘要对比；全部既有 v1/v2 pilot 测试 |
| H-06 | 本文证据边界与外部采样/归档要求；真实任务复核仍待执行 |

## 4. 工程质量结果

2026-09-16 的本地 Linux 验证通过 `cargo fmt --all -- --check`、全部目标/特性编译、
Clippy warnings denied 和全部目标/特性测试：428 项通过，0 失败，12 项依既有外部
生产者、凭据或收费模型合同忽略。LLVM 覆盖率为 95.77% Rust lines（22,511 行，
952 行未覆盖），超过 90% 门禁。

debug、release 和 staged Skill 的 full selfcheck 均为 338/338、complete/pass。
架构报告覆盖 160 个源码摘要和 1,501 条文件/行依赖，0 违规。
domain、config、adapters、application、interfaces 五个 Bazel 单元目标及根 CLI 构建通过；
CodeSpec 与 Knowledge map 均有效。机器可读输出与日志保存在
`target/pilot-phase-h-verification/`。

上一安装版与本次 release 对同一份未来窗口 v2 计划封存的完整 JSON 字节完全相同，
plan digest 均为 `sha256:1248ad170e29a75d0f0d3661521d4cbe39291131be1813e0a710be1d36389ebc`；
`compatibility.json` 保存两个二进制及输出摘要。另以临时身份生成 8 项任务、4+4 分层和
32 个运行单元的 v3 工程样例；release 成功封存，摘要为
`sha256:1b1c1cfb1bd6d5976f668c9ed771b3c28970ff91a43920265eca47d413fa281f`，
汇总按 8 个不同输入统计，不把 32 个 assignment 当作样本数。

此前用户要求的本地开发 Skill 已更新到 `~/.codex/skills/qualitygate-cli`，保留上一安装在
`~/.codex/skill-backups/qualitygate-cli-20260916T080707Z`。安装后 full selfcheck 为
338/338，已安装 release 摘要为
`sha256:f9c6d650c4a337760b111bb28cc78c4f899fe18e1ffa5bc54ca5e39c8c013852`；
8 项任务/4+4 分层的 v3 安装版烟测通过。`installation.json` 明确标记这是
dirty source 的本地 development install，而非公开发布版。

这些 fixture 不代替正式采样、签名或收益验收。没有重新运行真实模型、取得生产任务来源、
正式人工复核或开始七天观察；Miri、ASan 与外部 producer 仍属于各自独立门禁。
