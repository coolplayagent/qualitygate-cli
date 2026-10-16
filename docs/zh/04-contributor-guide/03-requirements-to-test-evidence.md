# 需求到测试证据

[English](../../en/04-contributor-guide/03-requirements-to-test-evidence.md) · [本卷目录](README.md)

规范需求位于 [CodeSpec requirements](../../../codespec/requirements/qualitygate-cli.md)。本章是稳定的
证据索引，取代带日期的实现台账和逐阶段进度记录。测试名代表可重复的仓库工程证据，不代表生产部署
或真实试点已验收。

## 快照、初始化与采集

| 需求领域 | 主要可重复证据 |
| --- | --- |
| 快照选择、执行、报告与退出语义 | `tests/cli.rs`、`tests/execution.rs`、`tests/policy.rs`、domain 单元测试 |
| 初始化与能力缺口 | `tests/init.rs` |
| 大文件测试 overlay、受保护容量授权、人读引导转义 | `tests/test_effectiveness.rs::raised_file_budget_reaches_both_test_effectiveness_executions`；`tests/policy_validation.rs::protected_file_capacity_is_bounded_authorized_and_used_for_both_policies`；`interfaces::render::tests::preflight_guidance_escapes_controls_without_changing_json_paths` |
| 超大文件采集仍为 incomplete，并报告实际字节数与最小重试预算 | `fixtures/golden/stress.json` 的 `snapshot-oversized` 精确约束 采集诊断；incomplete 结论与默认 2 MiB 限制不变 |
| 首次错误格式、HEAD/工作区建议性预检、大文件显式容量和策略约束 | `tests/init.rs::first_run_errors_honor_formats_and_show_an_action`、`init_preflights_tracked_ignored_and_untracked_large_files_without_reading_content`、`init_retains_incomplete_preflight_and_cannot_recommend_an_unsupported_budget`；`tests/large_repository.rs::legacy_large_blobs_are_acquired_explicitly_without_hiding_policy_or_source_changes`、`raised_file_budget_preserves_full_snapshot_bytes_and_digest_under_path_filtering`；`snapshot::git::tests::explicitly_permitted_large_blobs_use_nonempty_bounded_batches` |
| 缺失策略引导、原始命令参数、显式策略引用和 Windows 路径安全展示 | `tests/init.rs::first_run_errors_honor_formats_and_show_an_action`、`missing_custom_configuration_is_carried_into_the_command`（直接执行包含空格和单引号的返回参数）、`explicit_policy_reference_never_suggests_local_initialization`、`snapshot_guidance_depends_on_local_policy_and_selection`、`unreadable_or_malformed_configuration_does_not_suggest_init`；`paths::tests::displays_ordinary_windows_paths_without_verbatim_prefix`（Windows） |
| DIST-04 Skill 配置前置检查、自动初始化、失败停止与初始化后复查 | `tests/quality/skill_package.rs::skill_initialization_contract_requires_auto_init_stop_and_recheck` 验证入口元数据、只读启动示例、初始化边界和操作说明 |
| 大仓库获取和性能边界 | `tests/large_repository.rs`、`tests/benchmarks.rs`、`tests/policy_performance.rs` |

## 规则、策略与工具证据

| 需求领域 | 主要可重复证据 |
| --- | --- |
| CONSTRAINT-01 来源绑定清单、采用、快照覆盖与显式缺口 | `tests/constraint_coverage.rs`；`domain::constraint::tests` |
| OPERATOR-01 严格 v2 组合、完整事实、共享预算和有界来源观察记录 | `tests/operators.rs`；`domain::operator::tests`；`adapters::operators::tests`；既有 `tests/rule_authoring.rs`、`tests/custom_rules.rs` 保持 v1 回归 |
| 内置与项目规则 | `tests/issue9_rules.rs` 到 `tests/issue20_rules.rs`、`tests/custom_rules.rs`、`tests/rule_authoring.rs` |
| 规则类别、修改、候选、推广与生命周期 | `tests/rule_management.rs`、`tests/policy_categories.rs`、`tests/policy_candidates.rs`、`tests/policy_promotion.rs`、`tests/policy_lifecycle.rs` |
| 文件契约与独立测试反例 | `tests/file_contracts.rs`、`tests/test_effectiveness.rs` |
| Maven、Python、兼容性、覆盖率与 SARIF | `tests/maven.rs`、`tests/python.rs`、`tests/compatibility.rs`、`tests/coverage.rs`、`tests/sarif.rs` 及显式 live 目标 |
| 诊断棘轮 | `tests/ratchet.rs`、各语言/工具 ratchet 目标、`tests/c_family_ratchet.rs` |

## Agent 与任务验收

| 需求领域 | 主要可重复证据 |
| --- | --- |
| AGENT-01 — 绑定来源的 Agent 义务、按范围投影 Prompt、过期/缺失证据与冲突处理 | `tests/agent_contracts.rs`；`domain/agent_contract.rs` 范围单元测试 |
| AGENT-HOST-01 — Agent 注入、修复/恢复/切换、能力缺口、伪造报告、策略变化、不可变交付与规范路径边界 | `tests/agent_host.rs`；`tests/delivery.rs`，包括 `equivalent_repository_paths_cannot_bypass_delivery_confinement`；`tests/agent_loop.rs`；纯判定 `domain/agent_host_tests.rs` |
| 任务、人工验收、provenance 与 Git trailer | `tests/manual.rs`、`tests/provenance.rs`、`tests/case_provenance.rs`、`tests/git_trailers.rs` |
| Agent 反馈与有界修复闭环 | `tests/feedback.rs`、`tests/agent_loop.rs`、显式 repair/live 目标 |
| 试点封存、授权、证据与接受 | `tests/pilot_*`、`tests/pilot_seal/v8.rs` 到 `v10.rs`、pilot domain 测试 |
| Decision envelope 与可选判断 | `tests/decision_envelope.rs`、`tests/judgment_provider.rs` |

## 文档与分发

| 需求领域 | 主要可重复证据 |
| --- | --- |
| 架构、文档、站点与 Skill 发布 | `tests/quality/architecture.rs`、`documentation.rs`、`site.rs`、`skill_package.rs` |

规则回归测试保留正例、反例、非法输入、边界和未完成证据。报告适配器分别测试解析归一化与真实
producer 调用；live producer 不可用时不能写成集成通过。

## 证据解释

试点 fixtures 证明计划封存、来源工件、执行顺序、停止规则、逐尝试模型/执行记录、旧 Schema 兼容和
v10 非财务阈值的机器契约。它们不提供八个真实任务、外部 owner/reviewer 签名、观察窗口、服务商
原始日志或人工收益测量。

修改行为时，应把每项需求关联到最窄的纯/单元测试，并在适用时提供集成反例。记录不支持的平台、
不可用工具、性能环境和全部 incomplete。实际验证范围由当前 Git 提交与 gate 报告标识，而不是历史
散文状态行。
`tests/decision_envelope.rs` 还验证相同采集内容下全仓与两个不同 `--path` 的证据摘要不可互换；
`tests/coverage.rs` 验证两种范围及空分母场景的实际/配置模式元数据。

## 交付范围与证据身份

| 行为 | 可重复证据 |
| --- | --- |
| 选择器确定交付、不可变上下文、排除、空交付和无位置命令失败 | `tests/delivery_scope.rs`；选择器复验 argv 保持 verification digest，并拒绝已移除的 scope 参数。 |
| 不同策略排除分别绑定输入，并遵守两棵树预算 | `tests/policy_validation.rs`；相同排除保留独立并行检查。 |
| 内容摘要不能替代绑定范围的验证身份 | `tests/decision_envelope.rs`；跨平台往返 check/feedback，区分相同字节上的两种路径过滤。 |
| 覆盖率实际/配置模式及空分母 | `tests/coverage.rs`；显式 live JaCoCo/ESLint 对照保留 delivery 与核心 repository 契约。 |
| 无生产代码变化的适用性依赖保留上下文 | `tests/test_effectiveness.rs`。 |
| 历史与新增诊断、两种 producer 顺序中的实例计数及 producer 错误 | `tests/sarif.rs`、`tests/execution.rs`、`application::report_gate::tests`。 |
| 不可用的全仓复验标记 omitted，交付复验仍可执行 | 反馈单元测试与 `tests/decision_envelope.rs`。 |

## 运行时与预检

| 要求 | 单元 / 纯领域证据 | 集成 / 平台证据 |
| --- | --- | --- |
| ENV-39-01/02：离线指纹、运行时要求与解析诊断 | `config::runtime::tests`、`domain::doctor::tests` | `tests/doctor.rs`：离线 capabilities/Schema、兼容性反例 |
| ENV-39-03：指定计划与环境变量要求 | `config::plan::tests` | `tests/doctor.rs`：依赖展开、任务变量、缺失/空值、staged/diff/policy 选择；`tests/policy_promotion.rs`：活动签名策略 |
| ENV-39-04：静态边界与有界探针 | `application::doctor_context::tests` | `tests/doctor/probes.rs`：超时、超限、空版本、脚本/输入/策略变化、敏感输出 |
| ENV-39-05：分侧文件预算 | `snapshot::selected_preflight::tests` | `tests/doctor/budgets.rs`：仅 base 超限、2/8 MiB、最大容量、总量、受保护排除、index、计数与截断 |
| ENV-39-06：预检协议与可执行建议 | `domain::doctor::tests` | `tests/doctor.rs`：Schema 校验与建议 argv 重放 |
| ENV-39-07：Skill 流程、兼容表与原生 Shell | `tests/quality/skill_package.rs` | `pr-checks.yml`：保留调试断言/溢出检查、关闭调试符号的完整 Windows 测试命令；带空格路径的 PowerShell/Git Bash 验证 |

`cargo test --test doctor --all-features` 是独立于原生单元测试的集成目标。
Windows 实际 CI 结果、完整 selfcheck、架构源摘要/行号证据、Cargo/Bazel 和至少 90% 行覆盖率
仍须分别通过。Miri 验证纯领域，ASan 验证原生单元测试；Linux 结果不能替代 Windows Shell
实测。见[运行时与预检](../02-reference/06-runtime-and-preflight.md)。

---

[上一章：自检与 fixtures](02-selfcheck-and-fixtures.md) · [全书目录](../README.md) · [下一章：试点评估](04-pilot-evaluation.md)
