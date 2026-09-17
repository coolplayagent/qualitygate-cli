# 阶段 I：任务来源文件绑定

> 日期：2026-09-16；工程能力已实现；真实 8 项任务/7 天试点仍未启动。

## 1. 观察前准备

新试点从 [v4 清单模板](../templates/pilot/observation-v4.json)填写。仍使用已确认的
8 项任务、缺陷修复/重构各 4 项、七天、两种 Codex 模型组、两种流程和阶段 G 的结构化预算。
除 32 个 assignment 外，须在 `sources` 为每个独立 `input_id` 写一条来源记录：

```json
{
  "input_id": "bug-001",
  "kind": "issue",
  "source_id": "repository/issues/123",
  "path": "sources/bug-001.json",
  "digest": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "bytes": 1234,
  "selected_at": 1790000000
}
```

上述 ID、摘要、大小、时间仅示例，不能用于真实封存。`kind` 可为 `issue` 或 `commit`。
团队先将原始 issue/commit 证据快照放入清单所在的外部归档，再记录相对路径、准确字节数与
SHA-256。`source_id` 采用团队可复核的稳定身份；每个输入、身份、路径及摘要均须唯一。
来源文件每份 1–64 KiB，全部不超过 8 MiB。选入时间必须早于或等于预声明封存时间。

## 2. 封存与复验

`pilot seal` 验证来源文件是清单目录下的普通文件，拒绝路径逃逸、符号链接、缺失、长度或
摘要不符。`authorization-subject`、`pilot summarize` 与 `acceptance-subject` 每次调用
都重新读取并核对相同文件；读取后输出前再核对一次。来源的所有字段都由 plan seal 绑定，
归档文件改变后不能借旧封存或签名继续得出已验证结果。

汇总的 `source_audit` 按独立输入列出来源种类、ID、摘要及选入时间；模型×流程复制不增加
来源数。v4 沿用十项最终阈值和原有 owner/reviewer 签名主题语义；v1/v2/v3 已封存记录
保持原序列化和计划摘要。

摘要只验证所读文件与声明的字节一致。CLI 不能证明 issue/commit 的真实存在、独立性、
选入时间可信或文件内容未被人为编造；human owner/reviewer 需对照外部系统、Git 历史与
首次持久归档。没有具体八项任务和签名时，工程样例不能启动七天收益观察。

## 3. 需求到测试

| 要求 | 证据 |
|---|---|
| I-01/I-03/I-04 | `domain::pilot::tests::v4_seal_binds_one_unique_preselected_source_per_independent_task` |
| I-02/I-04 | `pilot_seal::cli_v4_rechecks_confined_source_artifacts_before_sealing_and_summary` |
| I-05 | 旧安装版与新版对相同 v3 清单的完整封存 JSON/摘要比较；全部既有 pilot 测试 |
| I-06 | 本文来源真实性边界及正式外部复核要求；实际来源尚待记录 |

## 4. 工程质量结果

2026-09-16 的本地 Linux 验证通过 `cargo fmt --all -- --check`、全部目标/特性编译、
Clippy warnings denied 和全部目标/特性测试：430 项通过，0 失败，12 项依既有外部
生产者、凭据或收费模型合同忽略。LLVM 覆盖率为 95.78% Rust lines（22,585 行，
954 行未覆盖），超过 90% 门禁。

debug、release 和 staged Skill 的 full selfcheck 均为 338/338、complete/pass。
架构报告覆盖 160 个源码摘要和 1,505 条文件/行依赖，0 违规。
domain、config、adapters、application、interfaces 五个 Bazel 单元目标及根 CLI 构建通过；
CodeSpec 与 Knowledge map 均有效。日志与机器可读输出保存在
`target/pilot-phase-i-verification/`。

上一安装版与本次 release 对同一份 8 项、32 单元 v3 计划封存的完整 JSON 字节完全相同，
plan digest 均为 `sha256:55d8150e1b46b671a48819264992e355c09dc8d0ada09982c5e47cd2315ff505`；
`compatibility.json` 保存两个二进制及输出摘要。另以临时身份生成 8 份不同来源文件、
4+4 任务配额和 32 个运行单元的 v4 工程样例；release 成功封存，摘要为
`sha256:636fcdd693e5df23786901d2b0442f9315076709125cdd99cdfbdafc4937364d`，
汇总按 8 个输入审计来源，不把 32 个 assignment 当作独立来源数。

此前用户要求的本地开发 Skill 已更新到 `~/.codex/skills/qualitygate-cli`，上一安装备份在
`~/.codex/skill-backups/qualitygate-cli-20260916T083649Z`。安装后 full selfcheck 为
338/338；release binary 摘要为
`sha256:5405b8ea61c1a2b4bb2fb41fef6306322677a01b795de806738e0987279abe6b`。
安装版的 8 来源 v4 封存、描述性汇总及启动授权主题烟测通过；`installation.json`
标记这是 dirty source 的本地 development install，不是公开发布版。

合成文件、任务和身份仅验证契约，不是正式试点样本。没有重新运行真实模型、获取正式
issue/commit 来源、签名、人工复核或开始七天观察；Miri、ASan 与外部 producer 仍属于
各自独立门禁。
