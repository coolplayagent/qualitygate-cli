# 需求到测试证据

[English](../../en/04-contributor-guide/03-requirements-to-test-evidence.md) · [本卷目录](README.md)

规范需求位于 [CodeSpec requirements](../../../codespec/requirements/qualitygate-cli.md)。本章是稳定的
证据索引，取代带日期的实现台账和逐阶段进度记录。测试名代表可重复的仓库工程证据，不代表生产部署
或真实试点已验收。

| 需求领域 | 主要可重复证据 |
| --- | --- |
| 快照选择、执行、报告与退出语义 | `tests/cli.rs`、`tests/execution.rs`、`tests/policy.rs`、domain 单元测试 |
| 初始化与能力缺口 | `tests/init.rs` |
| Issue 33 评审：大文件测试 overlay、受保护容量授权、人读引导转义 | `tests/test_effectiveness.rs::raised_file_budget_reaches_both_test_effectiveness_executions`；`tests/policy_validation.rs::protected_file_capacity_is_bounded_authorized_and_used_for_both_policies`；`interfaces::render::tests::preflight_guidance_escapes_controls_without_changing_json_paths` |
| 超大文件采集仍为 incomplete，并报告实际字节数与最小重试预算 | `fixtures/golden/stress.json` 的 `snapshot-oversized` 精确约束 Issue 33 的新提示；incomplete 结论与默认 2 MiB 限制不变 |
| Issue 33 首次错误格式、HEAD/工作区建议性预检、大文件显式容量和策略约束 | `tests/init.rs::first_run_errors_honor_formats_and_show_an_action`、`init_preflights_tracked_ignored_and_untracked_large_files_without_reading_content`、`init_retains_incomplete_preflight_and_cannot_recommend_an_unsupported_budget`；`tests/large_repository.rs::legacy_large_blobs_are_acquired_explicitly_without_hiding_policy_or_source_changes`、`raised_file_budget_preserves_full_snapshot_bytes_and_digest_under_path_filtering`；`snapshot::git::tests::explicitly_permitted_large_blobs_use_nonempty_bounded_batches` |
| Issue 35 缺失策略引导、原始命令参数、显式策略引用和 Windows 路径安全展示 | `tests/init.rs::first_run_errors_honor_formats_and_show_an_action`、`missing_custom_configuration_is_carried_into_the_command`（直接执行包含空格和单引号的返回参数）、`explicit_policy_reference_never_suggests_local_initialization`、`snapshot_guidance_depends_on_local_policy_and_selection`、`unreadable_or_malformed_configuration_does_not_suggest_init`；`paths::tests::displays_ordinary_windows_paths_without_verbatim_prefix`（Windows） |
| DIST-04 Skill 配置前置检查、自动初始化、失败停止与初始化后复查 | `tests/quality/skill_package.rs::skill_initialization_contract_requires_auto_init_stop_and_recheck` 验证入口元数据、只读启动示例、初始化边界和操作说明 |
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
`tests/decision_envelope.rs` 还验证相同采集内容下全仓与两个不同 `--path` 的证据摘要不可互换；
`tests/coverage.rs` 验证两种范围及空分母场景的实际/配置模式元数据。

交付范围与排除机制：`tests/delivery_scope.rs` 覆盖选择器自动确定交付范围与核心 API 全仓评估对照、历史大文件排除、
依赖上下文保留、所选策略来源、受保护文件冲突、空交付及命令失败。
选择器回归拒绝已移除的 scope 参数，并执行生成的复查 argv，验证 verification digest 保持一致。
`application::report_gate::tests` 验证报告范围与基线比较；`tests/quality.rs` 验证 Skill 包及架构。
`tests/coverage.rs` 与真实 JaCoCo/ESLint 验收对比增量分母、诊断与可复用核心的全仓契约；
`tests/policy_validation.rs` 验证不同 exclude 的输入独立绑定及两棵树预算，相同 exclude 保留独立并行检查。
`tests/decision_envelope.rs` 跨平台验证 check/feedback 范围摘要往返，并拒绝用内容摘要替代验证摘要。
`tests/test_effectiveness.rs` 利用未排除上下文保留“无生产代码变化”的适用性契约；SARIF 与 execution
回归区分历史诊断和交付诊断，同时保留生产器错误。
报告门禁单元测试和 SARIF CLI 回归覆盖保留历史实例并新增同标识诊断的两种生产器顺序；
反馈单元及 envelope 回归确保不可用的全仓复查仍标记 omitted，同时保留交付复查命令。

## Issue #39 运行时与预检证据

| 要求 | 单元 / 纯领域证据 | 集成 / 平台证据 |
| --- | --- | --- |
| ENV-39-01/02：离线指纹、运行时要求与解析诊断 | `config::runtime::tests`、`domain::doctor::tests` | `tests/doctor.rs`：离线 capabilities/Schema、兼容性反例 |
| ENV-39-03：指定计划与环境变量要求 | `config::plan::tests` | `tests/doctor.rs`：依赖展开、任务变量、缺失/空值、staged/diff/policy 选择；`tests/policy_promotion.rs`：活动签名策略 |
| ENV-39-04：静态边界与有界探针 | `application::doctor_context::tests` | `tests/doctor/probes.rs`：超时、超限、空版本、脚本/输入/策略变化、敏感输出 |
| ENV-39-05：分侧文件预算 | `snapshot::selected_preflight::tests` | `tests/doctor/budgets.rs`：仅 base 超限、2/8 MiB、最大容量、总量、受保护排除、index、计数与截断 |
| ENV-39-06：预检协议与可执行建议 | `domain::doctor::tests` | `tests/doctor.rs`：Schema 校验与建议 argv 重放 |
| ENV-39-07：Skill 流程、兼容表与原生 Shell | `tests/quality/skill_package.rs` | `pr-checks.yml`：带空格路径下的 Windows PowerShell/Git Bash 验证 |

`cargo test --test doctor --all-features` 是独立于原生单元测试的集成目标。
Windows 实际 CI 结果、完整 selfcheck、架构源摘要/行号证据、Cargo/Bazel 和至少 90% 行覆盖率
仍须分别通过。Miri 验证纯领域，ASan 验证原生单元测试；Linux 结果不能替代 Windows Shell
实测。见[运行时与预检](../02-reference/06-runtime-and-preflight.md)。
