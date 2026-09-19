# 需求到测试证据

[English](../../en/04-contributor-guide/03-requirements-to-test-evidence.md) · [本卷目录](README.md)

规范需求位于 [CodeSpec requirements](../../../codespec/requirements/qualitygate-cli.md)。本章是稳定的
证据索引，取代带日期的实现台账和逐阶段进度记录。测试名代表可重复的仓库工程证据，不代表生产部署
或真实试点已验收。

| 需求领域 | 主要可重复证据 |
| --- | --- |
| 快照选择、执行、报告与退出语义 | `tests/cli.rs`、`tests/execution.rs`、`tests/policy.rs`、domain 单元测试 |
| 初始化与能力缺口 | `tests/init.rs` |
| 大仓库获取和性能边界 | `tests/large_repository.rs`、`tests/benchmarks.rs`、`tests/policy_performance.rs` |
| 内置与项目规则 | `tests/issue9_rules.rs` 到 `tests/issue20_rules.rs`、`tests/custom_rules.rs`、`tests/rule_authoring.rs` |
| 规则类别、修改、候选、推广与生命周期 | `tests/rule_management.rs`、`tests/policy_categories.rs`、`tests/policy_candidates.rs`、`tests/policy_promotion.rs`、`tests/policy_lifecycle.rs` |
| 文件契约与独立测试反例 | `tests/file_contracts.rs`、`tests/test_effectiveness.rs` |
| Maven、Python、兼容性、覆盖率与 SARIF | `tests/maven.rs`、`tests/python.rs`、`tests/compatibility.rs`、`tests/coverage.rs`、`tests/sarif.rs` 及显式 live 目标 |
| 诊断棘轮 | `tests/ratchet.rs`、各语言/工具 ratchet 目标、`tests/c_family_ratchet.rs` |
| 任务、人工验收、provenance 与 Git trailer | `tests/manual.rs`、`tests/provenance.rs`、`tests/case_provenance.rs`、`tests/git_trailers.rs` |
| Agent 反馈与有界修复闭环 | `tests/feedback.rs`、`tests/agent_loop.rs`、显式 repair/live 目标 |
| 试点封存、授权、证据与接受 | `tests/pilot_*`、`tests/pilot_seal/v8.rs` 到 `v10.rs`、pilot domain 测试 |
| Decision envelope 与可选判断 | `tests/decision_envelope.rs`、`tests/judgment_provider.rs` |
| 架构、文档、站点与 Skill 发布 | `tests/quality/architecture.rs`、`documentation.rs`、`site.rs`、`skill_package.rs` |

Issue 规则测试保留正例、反例、非法输入、边界和未完成证据。报告适配器分别测试解析归一化与真实
producer 调用；live producer 不可用时不能写成集成通过。

试点 fixtures 证明计划封存、来源工件、执行顺序、停止规则、逐尝试模型/执行记录、旧 Schema 兼容和
v10 非财务阈值的机器契约。它们不提供八个真实任务、外部 owner/reviewer 签名、观察窗口、服务商
原始日志或人工收益测量。

修改行为时，应把每项需求关联到最窄的纯/单元测试，并在适用时提供集成反例。记录不支持的平台、
不可用工具、性能环境和全部 incomplete。实际验证范围由当前 Git 提交与 gate 报告标识，而不是历史
散文状态行。
