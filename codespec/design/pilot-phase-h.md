# 阶段 H：封存独立任务分层清单

> 日期：2026-09-16；需求：EVO-05-01、EVO-05-03；状态：工程实现，真实试点未启动。

## 交付契约

| 编号 | 要求与反例 |
|---|---|
| H-01 | 新试点采用 manifest schema v3，在观察前声明正整数 `protocol.task_mix`；各类型配额之和须恰好等于 `task_count`，不能用空配额、零值或溢出值封存。已确认的本轮清单固定 4 个缺陷修复和 4 个重构。 |
| H-02 | 封存按不同 `input_id` 计任务，而非按模型 × 流程 assignment 计数；实际每类任务数必须与封存前声明完全一致。把 32 个运行单元当成 8 个独立任务或改变一项任务类型须拒绝。 |
| H-03 | 不同 `input_id` 的 `task_id` 和 `task_digest` 各自不得重复；同一个输入在四个 cohort 中必须共享原有完整任务身份与真值。只改输入编号或复用任务契约不能增加样本数。 |
| H-04 | `pilot seal` 绑定配额、任务 ID、类型、契约摘要和全部计划字段；观察期间对其任一改动必须使旧 seal 失效。`pilot summarize` 以独立输入输出声明与实际分层及有界任务清单。 |
| H-05 | 已封存的 schema v1/v2 输入及其序列化、摘要与验收语义保持不变；v3 沿用 v2 结构化预算和十项最终阈值，不创造未经确认的新收益阈值。 |
| H-06 | 真正的独立问题、任务来源和样本抽取时间仍由团队及持久归档证明；不同 ID/摘要与 seal 只检测形式上的重复和事后漂移，不构成真实收益或七天观察证据。 |

## 数据和所有权

`templates/pilot/observation-v3.json` 固定本轮已确认的 8 项与 4+4 分层，是阶段 H 的
填写起点；阶段 I 起的新试点使用 v4 并补来源工件。v1/v2 模板留给已有计划和重放。
`task_mix` 是 `TaskKind → count` 的严格 JSON 对象。
`domain::pilot::validation` 纯验证版本、分层和独立任务身份，复用现有 seal 计算；
`domain::pilot::metrics` 纯生成 `sampling_audit`。没有新增 I/O、签名 owner 或进程执行。

`sampling_audit.tasks` 每个输入仅列一次 `input_id`、`task_id`、`task_kind`、`task_digest`；
整个 manifest 仍受 1 MiB 上限约束。任务清单和采样过程由外部 reviewer 对照真实 issue/commit
证据复核，CLI 无法证明语义上两个任务是否源于同一个故障。

## 测试与完成判据

领域单元测试覆盖有效 2 类分层、缺配额、错误配额、重复 ID/契约摘要、seal 后漂移；
`pilot_seal` CLI 集成测试覆盖有效封存、摘要清单和重复契约拒绝。
旧版二进制与新版二进制对相同 v2 输入的封存 JSON 与摘要须一致。
完整 Rust、覆盖率、自检、Bazel、架构与文档门禁通过仅证明工程实现；
正式 8 项任务、owner/reviewer、预算来源、持久归档和七天观察仍单独验收。
