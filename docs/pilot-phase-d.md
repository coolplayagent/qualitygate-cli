# 阶段 D：封存试点计划并准备真实观察

> 日期：2026-09-16；工程能力已实现；真实试点尚未启动。

## 1. 封存流程

新试点复制 `templates/pilot/observation-v4.json`，先填写协议、阶段 G 的结构化总预算、
阶段 H 的任务类型配额、阶段 I 的来源工件和全部 assignment，不填写 observation。v1/v2/v3 模板保留给已准备的
旧版清单及其原验收语义。
本轮已确认 8 个 input；每个 input 必须分配给 Codex 中、低模型组和 existing-tools、qualitygate
两种流程，共 32 个运行单元。owner、独立 reviewer、持久 archive、金额上限、七天窗口、
请求模型、harness、环境、工具、缓存、权限、任务/base/snapshot/策略摘要和预期问题必须在运行前确定。

```bash
qualitygate pilot seal --input pilot-plan.json --format json > observations.json
```

封存命令拒绝已经含有观察的输入、非 real 样本、缺少预期问题、事后排除、不完整模型×流程矩阵、
不同 input 的 cohort 矩阵不一致及治理字段缺失。v3+ 还拒绝类型配额不符或跨输入重复任务 ID/
契约摘要，v4 还要求来源文件字节验证。输出增加 `plan_seal`，团队应把首次输出及其
SHA-256 摘要保存到声明的外部归档。命令不写输入文件，也不创建 owner、reviewer 或归档身份。

## 2. 观察期间允许的变化

计划摘要绑定 schema、试点 ID、协议和 assignment。运行后可以填写服务商实际返回的
`cohort.actual_model` 并追加 observation；这两类字段不改变预先分组。修改请求模型、Agent、
harness、推理强度、流程、环境、工具、缓存、权限、任务、base、初始快照、策略/任务摘要、
必需检查、预期问题、可修复性或 exclusion 会使 seal 校验失败。

每次阶段性汇总仍使用：

```bash
qualitygate pilot summarize --input observations.json --format json
```

缺失运行、报告或人工数据继续保留在分母或 unknown 中。缺少 plan seal 的旧清单仍可做描述性
回放，但 `protocol_ready` 为 false。旧阶段 B/C 重放不会因增加摘要字段而变成真实试点证据。

## 3. 证据边界

`plan_seal` 是内容完整性检查，不是数字签名或可信时间戳。调用方可以重新生成另一份计划，
所以首次封存的外部归档记录仍是判断“观察前已固定”的权威证据。汇总继续输出
`descriptive_only` 和 `requires_external_review`，不能批准推广。

阶段 E 起，新试点还应按[启动授权流程](pilot-phase-e.md)由外部 human owner 签署精确封存主题；
seal 自身的完整性语义不变。

当前仓库没有具体 8 个生产任务、owner、reviewer、金额上限和持久归档，因此没有生成伪造的
正式封存清单，也没有开始七天时钟。工程验证使用临时 Git 仓库和合成身份，只证明上述拒绝与
绑定行为。

## 4. 需求到测试

| 需求 | 验证 |
|---|---|
| D-01/D-02 | `pilot_seal::cli_refuses_to_seal_after_observation_or_without_complete_governance` |
| D-03/D-04 | `domain::pilot::seal_rejects_unready_governance_ground_truth_matrix_and_late_observations` |
| D-05 | `domain::pilot::pre_observation_seal_binds_the_plan_but_allows_observed_model_and_results` |
| D-05/D-06 | `pilot_seal::cli_seals_before_observation_and_summary_rejects_later_plan_changes` |
| 既有汇总边界 | `pilot_summary` 的报告摘要、路径逃逸、缺失运行和 symlink 反例 |

## 5. 工程质量结果

2026-09-16 的本地 Linux 验证通过格式、全部目标/特性编译、Clippy warnings denied 和
全部目标/特性测试：410 项通过，12 项需要外部生产者、真实凭据或收费模型的测试按合同忽略。
LLVM 覆盖率为 95.81% Rust lines（21,559 行，904 行未覆盖），超过 90% 门禁。

full selfcheck 的 338 个 fixture 全部与 golden 一致。架构报告覆盖 156 个源摘要和 1,457 条
文件/行依赖，0 违规。domain、config、application、interfaces 四个 Bazel 单元目标通过，
根 CLI Bazel target 构建通过。CodeSpec 与 Knowledge map 均有效。自检报告保存在
`target/pilot-phase-d-verification/selfcheck.json`；其它结果来自本轮实际命令输出。

这些证据只覆盖工程实现。没有运行新的模型调用，没有生成正式生产任务封存记录，也没有启动
七天观察；Miri、ASan 和外部 producer 继续属于各自独立门禁。
