# 需求到测试证据

[English](../../en/04-contributor-guide/03-requirements-to-test-evidence.md) · [本卷目录](README.md)

规范需求位于 [CodeSpec requirements](../../../codespec/requirements/qualitygate-cli.md)。本章是稳定的
证据索引，取代带日期的实现台账和逐阶段进度记录。测试名代表可重复的仓库工程证据，不代表生产部署
或真实试点已验收。


## 公共评审知识包

`REVIEW-PACK-01`：`tests/issue64_rules.rs` 验证按需启用、warning 行为、语言范围、文件/行证据、
参数拒绝与修复；三套 issue64 selfcheck 保留独立 pass/fail golden。
`tests/review_pack.rs` 使用真实随包卡片和义务，验证 Schema/来源、独立夹具采用、角色选择和输入失效。
文档与 Skill 打包门禁验证发布资产。指令投影不是语义验证；这些测试不证明并发或生命周期正确性。

## Clean contract 维护

[CLEAN-01–07](../../../codespec/requirements/clean-contracts.md) 的可重复证据：

| 契约 | 测试证据 |
| --- | --- |
| 领域独立与 CLI 兼容 | `interfaces::value_parsers::tests`，`architecture_rejects_cli_framework_and_path_io_in_pure_owners` 及完整生产源码依赖图。 |
| 语言/模式共享约束与诊断归属 | `domain::rule_parameters::tests`、`domain::rule_guidance::tests`、配置参数/catalog 测试、适配器规则测试、`tests/issue64_rules.rs` 和完整 selfcheck。直接适配器的 C/C++ import 模式缺少支持时保持 incomplete。 |
| 精确摘要 | `domain::digest::tests`、`net::tests::maximum_provider_body_preserves_decoded_values_and_exact_wire_identity`、签名公钥摘要断言和策略绑定测试。 |
| 包清单与具名夹具登记 | `tests/selfcheck.rs`、配置 selfcheck/catalog-asset 测试和同一 checkout 的 corpus 比较；独立 golden 不由实际执行结果生成。 |
| 根路径解析的阻塞边界 | `application::repository_root::tests`、`paths::tests::repository_roots_keep_native_resolution_and_errors`、Unix symlink-root 测试和架构回归样例。 |
| 跨平台夹具前提 | policy/agent-host 夹具使用规范临时根目录并断言 Git 提交的 CRLF 原始字节；CLI 测试检查索引字节。command/probe 测试分别验证 Unix 内容篡改、Windows 拒写后属性变更导致失效。`env::tests` 使用真实环境入口共用的摘要函数验证 4,096/4,097 项和 1 MiB/+1 字节边界。 |

覆盖率、Bazel 与最终快照报告补充上述测试，不将启发式审查信号变为语义证明。
稳定工具链本地检查和 nightly Miri/ASan 结果单独记录。


## 原生语法审查规则

`NATIVE-SYNTAX-01` 对应 `adapters::native_syntax::tests`（语法形状、注释/字面量、无关名称、数值宏、
格式错误/过大/超时输入）与 `tests/issue18_rules.rs`、`tests/issue19_rules.rs`（CLI 显式启用、固定语言、
花括号初始化、已有头文件修改、新增行选择、解析失败、路径排除与拒绝正则覆盖）。三组 selfcheck 中
issue18/19 的样例及独立 golden 使用同一执行器。语法证据不等于编译器语义证据。

## 快照、初始化与采集

资源与并发审计另有集中维护的 [CodeSpec 验证记录](../../../codespec/test/README.md)。

| 需求领域 | 主要可重复证据 |
| --- | --- |
| 发布版本、Skill 包与网站链接一致性 | `tests/quality/site.rs::pages_site_links_resolve_and_match_the_release_version`、`pages_workflow_deploys_only_the_static_site_from_main`；`tests/quality/skill_package.rs::skill_package_contract_is_complete_and_matches_the_cli_version`；`tests/quality/documentation.rs` 文档链接和双语目录门禁 |
| YAML-EXPANSION-01：分配前展开预算、UTF-8、别名、映射键、深度、错误保留与恢复 | `config::yaml_budget::tests`；`tests/doctor/yaml.rs`；`tests/rule_authoring.rs::expanded_rule_yaml_is_rejected_before_generation_and_preserves_inputs`；`tests/policy_promotion.rs::external_yaml_trust_expansion_is_rejected_before_policy_store_access` |
| YAML-SCALAR-WORK-01：类型解析前的标量文本预算、数值别名、嵌套锚点成本、键/标签、编码与诊断位置 | `config::yaml_scalar_work::tests`；上述 YAML-EXPANSION-01 配置、任务、规则和外部信任回归中的数值场景 |
| 快照选择、执行、报告与退出语义 | `tests/cli.rs`、`tests/execution.rs`、`tests/policy.rs`、domain 单元测试 |
| SNAP-RENAME-01：确定性一一配对重命名来源、多余副本为新增、未配对旧路径为删除 | `snapshot::tests::rename_sources_are_consumed_once_in_path_order`：文本/二进制/空内容、保留原文件及权限；`tests/delivery_scope.rs::surplus_rename_copies_remain_added_and_subject_to_delivery_rules`：worktree/staged/path/diff 诊断及修复；`tests/benchmarks.rs::duplicate_content_rename_storm_consumes_each_old_path_once`：18,000 个相同来源及多余副本；既有采集超时测试继续执行 |
| SCOPE-INDEX-01：借用单次操作索引，保留别名、当前键优先及反馈范围语义 | `snapshot::scope_index::tests`：与单次查询逐项比较、重复别名、公开快照变更，以及 2,000 个变更/5,000 个未变更路径清单；既有 delivery、report、coverage 集成目标 |
| HISTORY-WORK-01：8 MiB 批量消息获取与声明历史共享 60 秒截止时间 | `snapshot::messages::tests`：Git 顺序、UTF-8 编码、换行/NUL 语义、非法 framing、提交数量及累计字节；`snapshot::history::tests`：执行前和完成时截止时间、内容驻留及解码前计量；`snapshot::git::tests::expired_git_parsing_stops_before_returning_partial_inputs` |
| 初始化与能力缺口 | `tests/init.rs` |
| XML-DISCOVERY-01 有界 Maven 建议性发现、独立元数据缺口及策略保留 | `config::discovery::maven_manifest::tests`：精确 32 层/100,000 节点边界、字面标记、XML 格式与命名空间；`tests/init/maven_xml.rs`：输入摘要、缺失策略时创建候选、已有策略字节保留、其他项目发现和修复恢复 |
| CONFIG-FILE-01 配置与发现输入限于有界普通文件，保留缺失策略语义，非法输入后不发布候选 | `config::tests::configuration_file_size_budget_is_inclusive_and_precedes_parsing`；`tests/init.rs::fifo_configuration_is_rejected_without_initialization_or_replacement`、`fifo_ignore_file_blocks_discovery_before_candidate_publication`（Unix）；既有不可读/格式错误策略回归 |
| RULE-ASSET-01：实际读取总量、文件/字节包含边界值、普通资产目标 | `config::catalog_assets::tests`：1 MiB/256 边界、稳定顺序、普通/特殊文件符号链接及元数据长度为零的反例；`tests/selfcheck.rs::underestimated_asset_lengths_are_incomplete_before_parsing_and_recover_after_repair`：Linux procfs 实际字节在解析前触发额度、结构化未完成及修复后的规则/语料身份恢复 |
| 大文件测试 overlay、受保护容量授权、人读引导转义 | `tests/test_effectiveness.rs::raised_file_budget_reaches_both_test_effectiveness_executions`；`tests/policy_validation.rs::protected_file_capacity_is_bounded_authorized_and_used_for_both_policies`；`interfaces::render::tests::preflight_guidance_escapes_controls_without_changing_json_paths` |
| 超大文件采集仍为 incomplete，并报告实际字节数与最小重试预算 | `fixtures/golden/stress.json` 的 `snapshot-oversized` 精确约束 采集诊断；incomplete 结论与默认 2 MiB 限制不变 |
| 首次错误格式、HEAD/工作区建议性预检、大文件显式容量和策略约束 | `tests/init.rs::first_run_errors_honor_formats_and_show_an_action`、`init_preflights_tracked_ignored_and_untracked_large_files_without_reading_content`、`init_retains_incomplete_preflight_and_cannot_recommend_an_unsupported_budget`；`tests/large_repository.rs::legacy_large_blobs_are_acquired_explicitly_without_hiding_policy_or_source_changes`、`raised_file_budget_preserves_full_snapshot_bytes_and_digest_under_path_filtering`；`snapshot::git::tests::explicitly_permitted_large_blobs_use_nonempty_bounded_batches` |
| 缺失策略引导、原始命令参数、显式策略引用和 Windows 路径安全展示 | `tests/init.rs::first_run_errors_honor_formats_and_show_an_action`、`missing_custom_configuration_is_carried_into_the_command`（直接执行包含空格和单引号的返回参数）、`explicit_policy_reference_never_suggests_local_initialization`、`snapshot_guidance_depends_on_local_policy_and_selection`、`unreadable_or_malformed_configuration_does_not_suggest_init`；`paths::tests::displays_ordinary_windows_paths_without_verbatim_prefix`（Windows） |
| DIST-04 Skill 配置前置检查、自动初始化、失败停止与初始化后复查 | `tests/quality/skill_package.rs::skill_initialization_contract_requires_auto_init_stop_and_recheck` 验证入口元数据、只读启动示例、初始化边界和操作说明 |
| DIST-05 Skill 路径、原生运行时选择和只读启动 | `tests/quality/skill_startup.rs` 对真实 CLI 执行打包的 Bash/PowerShell 示例，覆盖含空格路径、自定义配置、PATH 回退、保留/无效资产覆盖及缺失/损坏配置；Linux 额外验证非 Linux 平台不会选择 Linux 资产。初始化和发布契约继续由 `tests/quality/skill_package.rs` 验证。 |
| 大仓库获取和性能边界 | `tests/large_repository.rs`、`tests/benchmarks.rs`、`tests/policy_performance.rs` |

## 规则、策略与工具证据

| 需求领域 | 主要可重复证据 |
| --- | --- |
| GRAPH-WORK-01：索引化迭代依赖遍历与稳定策略顺序 | `config::dependencies::tests`：首个就绪节点等价、有环/未知/重复依赖、10,000 节点反向链、profile 闭包、待完成检查及禁用依赖；既有 `config::change_selection::tests` 和 `tests/command_applicability.rs` |
| BUILTIN-WORK-01：共享 10,000 条/8 MiB 预留预算与 30 秒分析截止时间 | `adapters::rules::builtin_limits_tests`：展开诊断字节、结构诊断数量、并行共享预留及多行变更范围；`adapters::diagnostic_budget::tests::builtin_workers_reject_expired_deadlines_before_retaining_findings`；`adapters::markers::tests`：索引邻接、标记边界与 2,000 个独立实体 |
| FACT-WORK-01：Schema 1/2/3 事实采集计入重复标记声明，并为独立选择器提供本地预算 | `adapters::custom_rules::selection::tests::every_rule_version_bounds_repeated_marker_declaration_bytes`、`selectors_without_an_external_budget_still_reject_excess_subjects`；既有共享命名来源及未完成采集测试 |
| COMMAND-SCOPE-01：显式语言/路径/输入命令选择、依赖保留及全局输入变化保守失效 | `config::change_selection::tests`；`tests/command_applicability.rs`：隔离 Git 仓库、混合语言、选择器、旧/新路径、空交付、默认兼容、策略变化、依赖、路径反馈与未完成执行；`c_headers_activate_both_c_and_cpp_consumers` 验证 `.h` 头文件的保守语言选择 |
| SYNTAX-REUSE-01：单次检查有界语法事实与内置声明语言默认值 | `adapters::syntax::cache::tests`；`adapters::structure_rules::tests::declared_builtin_languages_avoid_unrelated_syntax_without_overriding_policy`、`shared_syntax_facts_preserve_cross_file_moves_and_rule_diagnostics`；既有 `tests/issue9_rules.rs`、`tests/custom_rules.rs` 和语法/实体变更单元测试保留诊断、非法输入失败和跨文件移动语义 |
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
| SARIF-EXPANSION-01：累计引用文本、借用逻辑名、有界位置保留与结果序列化 | `adapters::reports::json::tests`：重复/跨 run 逻辑引用、转义符号、重复 URI 表；`adapters::reports::json_budget::tests`：精确与转义 JSON 预算；`tests/sarif.rs::excessive_location_expansion_is_incomplete_preserves_inputs_and_recovers`：交付未完成、原始证据保留、输入不变及修复恢复 |
| REPORT-EXPANSION-01：跨文件及 JSON 转义的有界诊断保留 | `adapters::reports::diagnostic_budget_tests`：ESLint、Checkstyle、PMD 共享路径、跨文件累计及转义；`adapters::reports::json_budget::tests`：追加前拒绝；`tests/diagnostic_expansion.rs`：未完成、原始字节/摘要、输入保留、恢复到正常违规或通过 |
| REPORT-SOURCE-01：完整路径优先、简写唯一的共享索引解析 | `application::report_sources::tests`：目录边界、Unicode、当前/基线隔离及穷举等价；既有 `application::report_gate::tests`：行范围、重命名及受影响范围；`tests/report_sources.rs`：诊断/覆盖率映射、歧义、证据保留及修复 |
| REPORT-RENAME-01：原始源位置验证后的基线重命名索引 | `application::report_sources::tests::baseline_rename_*`：未变更/复制路径、Unicode、空结果、首个目标与单步映射；`application::report_gate::tests::baseline_line_validation_precedes_rename_mapping_to_a_longer_current_source`；`tests/report_sources.rs::renamed_baselines_validate_original_lines_preserve_evidence_and_recover`：基线行号未完成、原始证据、重命名重复诊断及修复 |
| PYTHON-METADATA-01 安装元数据为普通文件、有界读取与未完成 producer 证据 | `application::python_install::tests`：精确 2 MiB 边界、超大文件、目录与符号链接隔离；`tests/execution/python_metadata.rs`：有界子进程验证 FIFO/socket/目录/超大产物、证据保留、依赖阻断及普通文件恢复；真实 pip 仍由 `tests/python.rs` 验证 |
| REPORT-COUNT-01 测试与覆盖率报告累计计数的溢出检查、证据保留与依赖阻塞 | `application::report_gate::tests::matched_entity_totals_reject_overflow_across_test_and_coverage_reports`；`tests/execution/report_counts.rs` |
| JUNIT-EVIDENCE-01 共享套件/结果校验、普通报告可选元数据、矛盾报告保留与依赖阻塞 | `adapters::reports::tests::junit_validates_optional_counters_in_every_suite`、`junit_rejects_contradictory_outcomes_and_detached_records`、`junit_nested_summaries_count_each_case_once`；`adapters::reports::test_cases::tests`；`tests/execution/junit_counts.rs`：未完成与违规区分、原始字节/摘要保留及修复恢复 |
| XML-DEPTH-01 递归报告解析前的迭代元素深度限制 | `adapters::xml_document::tests`：精确 32 层边界、拒绝 128/2,000/50,000 层与字面标记；`adapters::reports::tests::xml_reports_reject_excessive_nesting_before_tree_parsing`：六种 XML 报告格式；配对 JUnit 单元测试及 `tests/execution/junit_counts.rs`：超深报告证据保留与依赖阻塞 |
| XML-ADAPTER-01 五个 Maven/兼容性入口共用深度和节点限制 | `adapters::xml_document::tests`：精确 100,000 节点边界；`adapters::maven::tests::manifest_and_effective_model_obey_xml_depth_and_node_limits`、`adapters::maven_usage::tests::usage_model_obeys_xml_depth_and_node_limits`、`adapters::compatibility::tests::compatibility_and_inventory_obey_xml_depth_and_node_limits`；`tests/execution/maven_xml.rs` 与 `tests/compatibility/xml_limits.rs`：原始字节/摘要保留、依赖阻断及修复恢复 |
| XML-ATTRIBUTES-01 树解析前的元素级和文档级属性预算 | `adapters::xml_document::tests` 与 `config::discovery::maven_manifest::tests`：精确 1,024/100,000 边界、命名空间、字面文本和计数重置；报告、配对 JUnit、Maven、usage 与兼容性单元测试覆盖各 XML 入口；`tests/init/maven_xml.rs` 和 `tests/execution/junit_counts.rs`：缺口、策略/产物保留、依赖阻塞及恢复 |
| XML-NAMESPACE-01 XML 解析前共享的纯命名空间工作预算 | `domain::xml_budget::tests`：精确预算、算术检查、栈边界与兄弟节点累计；`adapters::xml_document::tests::xml_namespace_work_bounds_inherited_scopes_and_qualified_names` 和 `config::discovery::maven_manifest::tests::discovery_namespace_work_bounds_scope_expansion_and_name_resolution`：继承展开、名称查找和常规重复声明；各 XML 入口测试、`tests/init/maven_xml.rs` 与 `tests/execution/junit_counts.rs` 验证缺口、证据保留及恢复 |
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
| ASYNC-WORK-01：堆上命令调度、阻塞响应/测试摘要解析与报告渲染，证据不变 | `net::tests::maximum_provider_body_preserves_decoded_values_and_exact_wire_identity`；`interfaces::render::tests::report_output_preserves_evidence_and_limits_severity_filter_to_diagnostics`；`application::feedback::tests::retaining_feedback_preserves_report_and_exact_artifact_identity`；既有 CLI、feedback、decision-envelope 及 host 集成目标；`application::test_counts::tests::counting_keeps_captured_buffers_and_metadata_when_parsing_fails`；[有界栈调度回归](../../../tests/init/stack.rs) `rust_check_dispatch_preserves_outcomes_on_a_bounded_stack` |
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
| DOCTOR-56：有界摘要、私有临时报告保留、原有退出码、写入失败及完整 JSON 兼容性 | `application::doctor_output::tests`、`interfaces::doctor_summary::tests` | `tests/doctor/output.rs`：大量受阻诊断、独立目录、摘要/大小/Schema、非法配置与选择器、不可用存储及人类/JSON 格式 |
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

## LANGUAGE-SYNTAX-01：可选语言规则的结构证据

| Requirement | Evidence |
| --- | --- |
| 语法、绑定、插值、纯注释变更与有界未完成 | `adapters::language_syntax::tests` |
| 排除文档、属性和文件名误报 | `tests/internal_language_rules.rs::review_rules_ignore_documentation_properties_and_non_command_arguments` |
| 变更导入绑定与局部遮蔽 | `tests/internal_language_rules.rs::changed_go_imports_review_retained_calls_and_local_shadowing_removes_the_signal` |
| 不可变选定策略与源码 | `tests/internal_language_rules.rs::syntax_rules_use_selected_source_and_policy_not_dirty_worktree_bytes` |
| 错误语法、严格参数与路径排除 | `tests/internal_language_rules.rs::syntax_errors_and_regex_overrides_are_incomplete_but_excluded_sources_are_not_parsed` |
| 最小、典型和压力案例的独立节点类型预期 | `fixtures/golden/issue12-typical.json`, `fixtures/golden/issue14-stress.json`, `fixtures/golden/issue15-stress.json`, `fixtures/golden/issue16-typical.json`; full selfcheck |

## 面向场景的单元证据

`UT-SCENARIO-01` 使用独立的 `--lib --bins --all-features` 覆盖率门禁，门槛为 97.01%，
不计入集成测试 profile。下表对应可观察的行为契约；协议替身不能替代真实工具和端到端验收。

| 场景契约 | 单元证据 |
| --- | --- |
| 配置异常、有界发现、不可变候选包、规范来源变化 | `config::input_scenarios`、`config::project_inventory::scenario_tests`、`config::rule_authoring::tests`、`config::policy_candidates::tests`、`config::custom_validation::scenario_tests` |
| 语法与注释/字面量区分、测试框架发现、保留声明与依赖义务 | `adapters::markers::tests`、`adapters::language_syntax::tests`、`adapters::native_syntax::tests`、`adapters::structure_rules::tests`、`adapters::custom_rules::scenario_tests`、`adapters::git_trailers::tests` |
| 命令失败、超时、输出超限、输入/可执行文件变化；当前和基线报告独立绑定 | `application::commands::tests`、`application::generated_reports::tests`、`application::doctor::tests`、`application::rule_execution::tests`、`application::tests` |
| 历史缺失、配对评估超时、采集策略不同、实时信任与归档验证资产 | `application::policy_validation::tests`、`application::policy_promotion::tests`；使用临时 Git 和签名合成记录 |
| 有界 Agent 修复、契约/checkout 变化、求值器协议未完成与不可变交付 | `application::agent_host::scenario_tests`、`interfaces::cli::tests::agent_dispatch_keeps_invalid_evaluator_protocol_incomplete_with_retained_ledger`；使用固定可执行协议替身 |
| 独立负标签、弃权、校准审查优先级、失败尝试与试点矩阵完整性 | `application::judgment::tests`、`application::judgment::pilot::tests`、`domain::pilot::tests`、`application::pilot::tests`；不自动改变门禁，不宣称真实试点验收 |
| staged/diff/MR 身份、脏工作区、资源预算、报告与 CLI 边界 | `snapshot::merge_request::tests`、`snapshot::selected_preflight::scenario_tests`、`snapshot::delivery::tests`、`domain::report::comparison_tests`、`interfaces::render::tests`、二进制 `tests::process_boundary_distinguishes_help_parser_failure_success_and_execution_gap` |
| 小预算共享路径、跨文件和转义场景保留公开入口真实 16 MiB 边界 | `adapters::reports::diagnostic_budget_tests`、`adapters::reports::json_budget::tests` |
| 子进程组 SIGTERM 不终止 runner；CLI 输出未完成证据并返回 2，随后通过的检查返回 0 | `runner::tests::child_group_sigterm_does_not_terminate_the_runner_or_its_next_command`；集成 `tests/execution.rs::unix::child_group_sigterm_leaves_cli_alive_with_an_incomplete_report_and_exit_two` |
