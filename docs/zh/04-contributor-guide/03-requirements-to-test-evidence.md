# 需求到测试证据

[English](../../en/04-contributor-guide/03-requirements-to-test-evidence.md) · [本卷目录](README.md)

规范需求位于 [CodeSpec requirements](../../../codespec/requirements/qualitygate-cli.md)。本章是稳定的
证据索引，取代带日期的实现台账和逐阶段进度记录。测试名代表可重复的仓库工程证据，不代表生产部署
或真实试点已验收。

## 快照、初始化与采集

| 需求领域 | 主要可重复证据 |
| --- | --- |
| 快照选择、执行、报告与退出语义 | `tests/cli.rs`、`tests/execution.rs`、`tests/policy.rs`、domain 单元测试 |
| SNAP-RENAME-01：确定性一一配对重命名来源、多余副本为新增、未配对旧路径为删除 | `snapshot::tests::rename_sources_are_consumed_once_in_path_order`：文本/二进制/空内容、保留原文件及权限；`tests/delivery_scope.rs::surplus_rename_copies_remain_added_and_subject_to_delivery_rules`：worktree/staged/path/diff 诊断及修复；`tests/benchmarks.rs::duplicate_content_rename_storm_consumes_each_old_path_once`：18,000 个相同来源及多余副本；既有采集超时测试继续执行 |
| 初始化与能力缺口 | `tests/init.rs` |
| CONFIG-FILE-01 配置与发现输入限于有界普通文件，保留缺失策略语义，非法输入后不发布候选 | `config::tests::configuration_file_size_budget_is_inclusive_and_precedes_parsing`；`tests/init.rs::fifo_configuration_is_rejected_without_initialization_or_replacement`、`fifo_ignore_file_blocks_discovery_before_candidate_publication`（Unix）；既有不可读/格式错误策略回归 |
| RULE-ASSET-01：实际读取总量、文件/字节包含边界值、普通资产目标 | `config::catalog_assets::tests`：1 MiB/256 边界、稳定顺序、普通/特殊文件符号链接及元数据长度为零的反例；`tests/selfcheck.rs::underestimated_asset_lengths_are_incomplete_before_parsing_and_recover_after_repair`：Linux procfs 实际字节在解析前触发额度、结构化未完成及修复后的规则/语料身份恢复 |
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
| RULE-OUTPUT-01: bounded diagnostic records, serialized bytes and replay expansion | `adapters::diagnostic_budget::tests`; `tests/custom_rules/output_budget.rs`: v1/v2/v3 incompleteness, retained findings, expression/fixed allowance and repair |
| OPERATOR-01 严格 v2 组合、完整事实、共享预算和有界来源观察记录 | `tests/operators.rs`；`domain::operator::tests`；`adapters::operators::tests`；既有 `tests/rule_authoring.rs`、`tests/custom_rules.rs` 保持 v1 回归 |
| OPERATOR-SET-01 有界集合索引、首次来源/顺序、Unicode 精确相等、重复值与完整空集合操作数 | `tests/operators/v3.rs::v3_file_correspondence_scales_without_relaxing_shared_budgets`；`adapters::operators_v3::tests`：成员关系对照、稳定观察记录、20,000 个重复标量、小左集合/大右集合子集、无用索引避免与索引操作/扫描耗尽 |
| OPERATOR-02 | `tests/operators/v3.rs` (via `tests/operators.rs`): named source correspondence, unchanged tests, staged/path scope, source/policy changes, schema/generation compatibility, missing facts and exhaustion; `domain::operator_v3::tests` and `adapters::operators_v3::tests`: lexical typing, projection, set operations, UTF-8, provenance and shared budgets; `adapters::custom_rules::selection::tests`: shared acquisition limits, reservations before copying and inherited syntax deadlines; `v3_acquisition_exhaustion_keeps_prior_violations_and_never_claims_complete_inputs`: incomplete acquisition retains fixed findings. |
| 内置与项目规则 | `tests/issue9_rules.rs` 到 `tests/issue20_rules.rs`、`tests/custom_rules.rs`、`tests/rule_authoring.rs` |
| OPERATOR-PORTABLE-01：Python/TypeScript 文件对应规则无需语言运行时 | `tests/operators/portable.rs::python_and_typescript_lambdas_need_only_the_cli_and_git`（Unix，通过 `tests/operators.rs`）：子进程 `PATH` 仅保留 Git，验证语言工具不可用、Schema 校验、未修改对应文件、违规、快照变化及来源过期；不执行项目测试，也不证明 Windows 运行时隔离 |
| 规则类别、修改、候选、推广与生命周期 | `tests/rule_management.rs`、`tests/policy_categories.rs`、`tests/policy_candidates.rs`、`tests/policy_promotion.rs`、`tests/policy_lifecycle.rs` |
| 文件契约与独立测试反例 | `tests/file_contracts.rs`、`tests/test_effectiveness.rs` |
| Maven、Python、兼容性、覆盖率与 SARIF | `tests/maven.rs`、`tests/python.rs`、`tests/compatibility.rs`、`tests/coverage.rs`、`tests/sarif.rs` 及显式 live 目标 |
| PYTHON-METADATA-01 安装元数据为普通文件、有界读取与未完成 producer 证据 | `application::python_install::tests`：精确 2 MiB 边界、超大文件、目录与符号链接隔离；`tests/execution/python_metadata.rs`：有界子进程验证 FIFO/socket/目录/超大产物、证据保留、依赖阻断及普通文件恢复；真实 pip 仍由 `tests/python.rs` 验证 |
| REPORT-COUNT-01 测试与覆盖率报告累计计数的溢出检查、证据保留与依赖阻塞 | `application::report_gate::tests::matched_entity_totals_reject_overflow_across_test_and_coverage_reports`；`tests/execution/report_counts.rs` |
| JUNIT-EVIDENCE-01 共享套件/结果校验、普通报告可选元数据、矛盾报告保留与依赖阻塞 | `adapters::reports::tests::junit_validates_optional_counters_in_every_suite`、`junit_rejects_contradictory_outcomes_and_detached_records`、`junit_nested_summaries_count_each_case_once`；`adapters::reports::test_cases::tests`；`tests/execution/junit_counts.rs`：未完成与违规区分、原始字节/摘要保留及修复恢复 |
| XML-DEPTH-01 递归报告解析前的迭代元素深度限制 | `adapters::xml_document::tests`：精确 32 层边界、拒绝 128/2,000/50,000 层与字面标记；`adapters::reports::tests::xml_reports_reject_excessive_nesting_before_tree_parsing`：六种 XML 报告格式；配对 JUnit 单元测试及 `tests/execution/junit_counts.rs`：超深报告证据保留与依赖阻塞 |
| XML-ADAPTER-01 五个 Maven/兼容性入口共用深度和节点限制 | `adapters::xml_document::tests`：精确 100,000 节点边界；`adapters::maven::tests::manifest_and_effective_model_obey_xml_depth_and_node_limits`、`adapters::maven_usage::tests::usage_model_obeys_xml_depth_and_node_limits`、`adapters::compatibility::tests::compatibility_and_inventory_obey_xml_depth_and_node_limits`；`tests/execution/maven_xml.rs` 与 `tests/compatibility/xml_limits.rs`：原始字节/摘要保留、依赖阻断及修复恢复 |
| COVERAGE-THRESHOLD-01 精确计数判定、小数阈值相等、可选分支与空测量集合 | `domain::coverage::tests`；`tests/coverage.rs::coverage_thresholds_use_exact_counts_before_rounding_percentages` |
| LCOV-SUMMARY-01 源文件与声明/观测计数诊断、原始证据保留和未完成判定 | `adapters::reports::tests::lcov_sections_merge_by_identity_and_validate_source_summaries` 在合法源文件段之后验证四种计数器；`tests/coverage_llvm_live.rs::rust_lcov_reports_require_measured_branches_and_consistent_source_summaries` 在独立 `coverage-producers` 作业中执行锁定的 LLVM 导出器。 |
| OUTPUT-INPUT-01 报告及项目输出与输入冲突时准备前拒绝、静态预检、基线保护、报告新鲜度与命令启动完整性 | `tests/execution/preparation.rs`；`tests/cli.rs` 与 `tests/execution.rs` 的旧报告回归 |
| OUTPUT-PATH-01 规范化输出名唯一、基线清单独立、合法别名保留与报告计数不重复 | `config::tests::report_output_names_are_unique_after_normalization_without_rewriting_policy` 及相邻基线/项目单元测试；`tests/execution/output_aliases.rs` |
| SHARED-EXECUTION-01 基线派生范围绑定、不可变执行输入/摘要共享、独立工作区与输入替换检测 | `snapshot::tests::derived_baseline_preserves_binding_and_isolates_shared_execution_inputs`：基线/当前摘要绑定、创建时拒绝等长内容篡改及永久失败；`snapshot::input_guard::tests`；`tests/ratchet.rs`；`tests/compatibility.rs` |
| 诊断棘轮 | `tests/ratchet.rs`、各语言/工具 ratchet 目标、`tests/c_family_ratchet.rs` |
| RATCHET-EVIDENCE-01 基线失败后保留当前文件/stdout 报告、双方错误与依赖阻塞 | `tests/ratchet.rs::ratchet_retains_current_file_and_stdout_reports_when_baseline_fails`：两种依赖基线的模式、多份当前报告、组合失败与修复；`ratchet_baseline_failures_timeouts_mutation_and_tool_mismatch_are_incomplete`：基线执行及身份验证失败后的原始字节/摘要保留 |
| INPUT-PROTECTION-01 Windows 共享读取、阻止原地写入与替换、属性永久失效、句柄数量限制及创建失败/释放时关闭 | `snapshot::input_guard::tests`：原地改写恢复反例、共享冲突、并存校验器、已有写入句柄与初始检查失败；`snapshot::tests`；`application::doctor_context::tests::exhausted_shared_budget_and_invalid_guard_never_start_another_probe`；`tests/selfcheck.rs`；`.github/workflows/windows-snapshot.yml` |
| INPUT-PROTECTION-01 写入被拒绝保留输入已验证状态、非零退出码和原始共享冲突错误；实际属性变化使覆盖率、基线和成对测试执行失效 | `tests/coverage.rs::native_coverage_repairs_preserve_raw_reports_and_reject_inconsistent_or_absent_evidence`；`tests/ratchet.rs::ratchet_baseline_failures_timeouts_mutation_and_tool_mismatch_are_incomplete`；`tests/test_effectiveness.rs::execution_gaps_and_non_assertion_failures_never_establish_effectiveness` |

## Agent 与任务验收

| 需求领域 | 主要可重复证据 |
| --- | --- |
| AGENT-01 — 绑定来源的 Agent 义务、按范围投影 Prompt、过期/缺失证据与冲突处理 | `tests/agent_contracts.rs`；`domain/agent_contract.rs` 范围单元测试 |
| AGENT-HOST-01 — Agent 注入、修复/恢复/切换、能力缺口、伪造报告、策略变化、不可变交付与规范路径边界 | `tests/agent_host.rs`；`tests/delivery.rs`，包括 `equivalent_repository_paths_cannot_bypass_delivery_confinement`；`tests/agent_loop.rs` 验证修复 fixture 完整读取 Prompt；`runner::tests::closed_stdin_is_incomplete_even_when_the_child_exits_successfully` 保留输入交付失败语义；纯判定 `domain/agent_host_tests.rs` |
| 任务、人工验收、provenance 与 Git trailer | `tests/manual.rs`、`tests/provenance.rs`、`tests/case_provenance.rs`、`tests/git_trailers.rs` |
| Agent 反馈与有界修复闭环 | `tests/feedback.rs`、`tests/agent_loop.rs`、显式 repair/live 目标 |
| FEEDBACK-WORK-01：延后诊断 JSON 构建，保留按顺序容纳成员的语义 | `domain::feedback::tests::smaller_later_members_and_checks_survive_an_exhausted_inventory`：跳过超大成员后保留较小成员及标识更短的后续检查，独立超时证据与省略数不变；现有反馈单元/集成测试保持 UTF-8 截断、报告指针、完整命令、过滤及门禁权威。性能比较单独记录实际源码与测量范围。 |
| 试点封存、授权、证据与接受 | `tests/pilot_*`、`tests/pilot_seal/v8.rs` 到 `v10.rs`、pilot domain 测试 |
| Decision envelope 与可选判断 | `tests/decision_envelope.rs`、`tests/judgment_provider.rs` |

## 文档与分发

| 需求领域 | 主要可重复证据 |
| --- | --- |
| CI-NODE-01：Node 24 action 运行时，显式关闭 npm 自动缓存 | `.github/workflows/pr-checks.yml` 的 `eslint-project` 实际执行 ESLint ratchet 与修复测试；`.github/workflows/release.yml` 使用相同 setup 输入。`tests/quality/skill_package.rs` 验证发布打包，不执行发布。 |
| CI-WINDOWS-01：在原有作业预算内完整分组 Windows 目标 | `.github/workflows/pr-checks.yml` 在原生作业分别执行核心目标与互补的集成 glob；逐目标比较各组与 `--all-targets` 的 `cargo test ... -- --list` 清单，包含忽略测试。三组与 PowerShell/Git Bash 预检均须通过，取消仍属未完成。 |
| CI-ARTIFACT-01：Node 24 证据上传与静态 Pages 交付 | `tests/quality/site.rs::pages_workflow_deploys_only_the_static_site_from_main` 绑定站点路径及隐藏文件排除；`tests/quality/skill_package.rs` 校验打包。架构/覆盖率实际上传、`.github/workflows/selfcheck.yml` 三平台产物及 `.github/workflows/pages.yml` 部署需分别保留执行证据。 |
| CI-INVENTORY-01：由 Cargo 发现集成目标 | `tests/quality/gates.rs::documentation_repository_integration_gate_covers_cargo_targets` 使用有界、离线的 Cargo 元数据核对仓库策略，拒绝目标遗漏或过滤。`qualitygate.yaml` 与 CI `rust-integration` 作业使用字面量 `*` 目标选择器；实际执行与 `--list` 清单验证所选测试。忽略测试仍需在文档规定的环境中单独执行。 |
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
| ENV-39-07：Skill 流程、兼容表与原生 Shell | `tests/quality/skill_package.rs` | `pr-checks.yml`：保留调试断言/溢出检查、关闭调试符号的完整 Windows 目标分组；带空格路径的 PowerShell/Git Bash 验证 |
| EXECUTABLE-FILE-01：打开前校验普通文件，保留大小/摘要限制与符号链接兼容性 | `runner::identity::tests` | `tests/agent_host.rs::fifo_evaluator_is_rejected_before_opening_or_launching_the_agent`（Unix）：有界旧代码反例、未完成台账、无 Agent 启动或交付 |

`cargo test --test doctor --all-features` 是独立于原生单元测试的集成目标。
Windows 实际 CI 结果、完整 selfcheck、架构源摘要/行号证据、Cargo/Bazel 和至少 90% 行覆盖率
仍须分别通过。Miri 验证纯领域，ASan 验证原生单元测试；Linux 结果不能替代 Windows Shell
实测。见[运行时与预检](../02-reference/06-runtime-and-preflight.md)。

---

[上一章：自检与 fixtures](02-selfcheck-and-fixtures.md) · [全书目录](../README.md) · [下一章：试点评估](04-pilot-evaluation.md)

## RESOURCE-43: Default document and media exclusions

| Requirement | Evidence |
| --- | --- |
| Versioned suffix classification, precedence and bounded reason counts | `domain::resource_exclusions::tests` |
| 复用采集列表、限制合并清单大小、排除未选中的中间 HEAD 路径 | `snapshot::limits::tests::shared_exclusion_inventory_bounds_the_union_and_ignores_bootstrap_omissions`; `tests/delivery_scope/resources.rs::resource_inventory_uses_acquired_trees_without_unselected_head_entries` |
| Exclude before per-file/aggregate budgets; base-only, changed, renamed, deleted, staged and diff resources | `tests/delivery_scope/resources.rs::default_resources_are_excluded_before_budgets_in_every_local_selector`, `aggregate_resources_do_not_consume_content_budget_and_unknown_types_still_fail` |
| Retention, opt-out, protected inputs, explicit exclusion conflicts, strict policy and selected policy | `tests/delivery_scope/resources.rs::retain_disable_and_protected_inputs_restore_acquisition_without_bypassing_limits`, `policy_selection_and_strict_configuration_apply_to_default_exclusions` |
| Missing command evidence stays incomplete; retained and declared tool dependencies remain available | `tests/delivery_scope/resources.rs::command_dependencies_need_retention_and_missing_evidence_stays_incomplete` |
| Policy/inventory identity, history consistency and timeout | `tests/delivery_scope/resources.rs::resource_policy_and_inventory_bind_snapshot_identity_and_history` |
| MR resources do not hide source violations | `tests/merge_request.rs::merge_request_excludes_large_documents_from_source_without_hiding_source_findings` |
| Bootstrap/custom source protection and retained resource read failures | `tests/custom_rules.rs::custom_rule_sources_are_protected_even_with_a_document_suffix`; `tests/delivery_scope/resources.rs::bootstrap_protects_configuration_with_a_resource_suffix`, `restored_resources_do_not_bypass_confined_path_failures` (Unix) |
| Complete counts with bounded details and restored over-budget resources | `snapshot::selected_preflight::tests::resource_counts_and_protected_overrides_survive_detail_truncation` |
