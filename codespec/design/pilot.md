# 真实仓库试点设计

**当前协议为 manifest v10，正式试点待启动。** 项目已具备封存计划、复核逐次尝试和验证独立签名的工程能力。具体真实任务及真值、正式 owner 和 reviewer、持久归档、32 份初始 full 报告与启动签名仍待团队提供。仓库夹具、历史回放和受控模型实验各自保留来源。真实收益须在约定窗口结束后由独立 reviewer 验收。[演进需求与当前状态](../requirements/agent-acceptance-evolution.md#7-剩余验收与收尾边界) · [v10 观察清单模板](../../templates/pilot/observation-v10.json)

## 本轮协议与收尾条件

2026-09-16 确认的本轮方案包含 4 个缺陷修复和 4 个重构，观察 7 天。每个独立输入在两个 Codex 请求模型与两种工作流（既有工具、Qualitygate）下各运行一次，共 32 个 assignment。每单元最多 3 次尝试、累计 30 分钟，连续 2 次无可验证进展后停止，总上限为 96 次尝试、16 小时累计运行时间。这些数值属于本轮方案。其他试点按各自封存的分层、配额和停止参数执行。[本轮参数与阈值](../requirements/agent-acceptance-evolution.md#5-实施阶段与试点输入)

| 收尾编号 | 责任人与执行顺序 | 可核验的退出条件 |
| --- | --- | --- |
| C-01 | 团队选定输入与真值 | 8 项真实任务各有独立 issue/commit 来源、不可变基准、预审的目标与预期问题、任务契约和验证资产。历史回放、故障注入、副本及 Agent 自拟真值均不能计入样本。 |
| C-02 | 团队固定比较条件和证据保管人 | 确认 human owner、不同的独立 reviewer、仓库外持久归档和信任库，固定工具、环境、缓存、模型推理强度及交替顺序。 |
| C-03 | CLI 封存，owner 授权 | 为每个 assignment 取得真实的初始 full 报告。在无 observation 时执行 `pilot seal`，把输出存入持久归档。owner 从 `pilot authorization-subject` 取得精确主题并在外部签名，授权覆盖整个窗口。 |
| C-04 | 外部 harness 运行两种工作流 | 在七天窗口中按封存顺序执行，保留失败、超时、放弃和不完整尝试，包括模型与执行记录、快照及对应报告。缺证据仍占分母，费用观察可以缺失。 |
| C-05 | 独立 reviewer 复核并签署最终决定 | 窗口结束后核对来源文件、原始模型/进程/时钟日志、人工活跃时间和独立问题标签。`pilot acceptance-subject` 根据完整证据导出主题。[八项非财务检查](#阶段-o)均达标可签接受，完整证据也可签拒绝。缺证和未签状态保留。`pilot summarize` 重验所提供的授权与验收记录。 |
| C-06 | 对照 EVO-03 核实推广范围 | 本轮授权的 Terra 与 Luna 属于同一 Codex 产品，仅覆盖跨模型比较。至少两个实际 Agent 产品的目标仍缺第二产品授权、可比运行和独立记录。 |

此表沿用原收尾编号，引用时写“收尾 C-01”等，以区别下文[阶段 C](#阶段-c) 的案例来源条款。摘要绑定归档字节，原始任务真实性、模型路由、可信时间与人工标签由外部复核者确认。领域层计算协议和指标，配置层有界读取清单与报告，应用层编排外部证据复核，签名适配器校验认证记录。CLI 提供封存、导出主题和汇总能力，签名密钥由团队管理。[协议与验收判定](../../src/qualitygate/pilot_acceptance/pilot_contracts/acceptance.rs) · [执行编排](../../src/qualitygate/pilot_acceptance/pilot_workflow.rs)

## 版本与证据导航

| 查阅目的 | 对应阶段 |
| --- | --- |
| 建立配对基线、收集反馈、形成可复核的案例 | [A 基线](#阶段-a)、[B 修复闭环](#阶段-b)、[C 案例与汇总](#阶段-c) |
| v1 封存与两次独立签名 | [D 计划封存](#阶段-d)、[E 启动授权](#阶段-e)、[F 最终验收](#阶段-f) |
| v2–v5 预算、独立任务、来源与顺序 | [G 总预算](#阶段-g)、[H 任务分层](#阶段-h)、[I 来源工件](#阶段-i)、[J 交替顺序](#阶段-j) |
| v6–v9 复核尝试、模型路由与时间 | [K 停止规则](#阶段-k)、[L 单元模型采集](#阶段-l)、[M 逐次模型采集](#阶段-m)、[N 执行回执](#阶段-n) |
| v10 八项非财务阈值、旧版本重放 | [O 非财务验收](#阶段-o) |

## 任务基线与修复证据

### 阶段 A

**固定可比较的基线。** 团队在观察前锁定每个案例的任务、工具与验证条件，并保存原始输出。

| 要求 | 交付与验收 |
| --- | --- |
| A-01 / EVO-05-01 | 预先记录项目、负责人、样本、周期、预算和验收阈值。按准备中、输入已确认、基线已采集、阶段已验收推进。 |
| A-02 / EVO-01-01–03 | rust-v1 模板用于具备 Cargo manifest、锁文件和独立 `tests/regression.rs` 的项目。任务经团队审查测试覆盖并纳入选定策略提交。修复要求旧代码完成测试且断言失败、新代码通过。重构允许相同行为测试两侧均通过，额外兼容性由项目声明。编译失败、零测试和超时都缺少行为证据。 |
| A-03 / EVO-05-02 | 不同流程在同一案例使用相同策略、任务、工具、环境和比较基准。保留既有工具的原始报告、初始失败与未完成结果，独立归因新增检测。 |
| A-04 / EVO-03-01 | Codex 的两个请求模型 `gpt-5.6-terra`、`gpt-5.6-luna` 均用 `medium` 推理强度和同一 harness。记录产品、harness 版本、实际连接、命令执行及失败。服务商未提供可验证路由时，实际模型标为未知。 |
| A-05 / EVO-05-03–04 | 列明能力缺口。缺必需输入时阶段验收保持未完成。真实修复、跨产品兼容、团队收益以及既有 M1–M4 均有各自的验收证据。 |

按案例 ID 记录来源仓库、真实开发/历史回放/受控夹具分类、任务类型及摘要、base/head、策略提交、命令、工具版本、环境、时间和预算。实际基线使用 `qualitygate check --diff <base>..<head> --task <task> --policy-ref <policy> --profile full --format json`，保留完整报告、生产者日志及摘要引用。两次 check 可验证修复前后的行为，逐测试文件证明另需显式选择 `test_effectiveness` 与兼容生产者。外部 harness 和复核者核实来源与完整性。[Rust 模板](../../templates/pilot/rust-v1/README.md) · [基线回归](../../tests/pilot_baseline.rs)

离线回归验证 `parse_task`、`Plan::build`、任务与快照绑定、旧失败/新通过、重构双通过、零测试及策略变更拒绝。临时仓库结果标为 fixture。实际模型接入由默认忽略的有费用探测显式运行。每个模型一次、180 秒、每输出流最多 16 MiB，使用现有 Rust runner、独立的固定提交克隆和只读会话，忽略用户配置和 rules。探测要求成功执行 CLI 版本命令、读取契约并正确解释 0/1/2 与验收边界。默认 bwrap 受限时，可显式选择当前版本支持的只读 Landlock 并记录差异。保存原始事件、回答、参数、用量和失败。格式解析用离线正反例验证，归档重审时核对路径、大小和摘要，另存验证器摘要及原判定，保留 harness 原始索引。[模型接入探测与重审](../../tests/pilot_codex_live.rs)

### 阶段 B

**让 Agent 据实修复并在最终快照复验。** `check --feedback` 先持久化完整报告，再提供有界 JSON 视图。领域层做纯投影，应用层绑定已保存的报告，接口层选择呈现方式。既有 `evaluate`、完整报告 schema v1 和退出码 0/1/2 继续生效。

| 要求 | 交付与验收 |
| --- | --- |
| B-01 / EVO-01 | 复用 A 阶段两种任务契约和受保护的独立测试，分别保留修复旧失败/新通过及重构双通过证据。 |
| B-02 / EVO-02-01–03 | 反馈视图独立版本化，默认 32 KiB，可选 4–256 KiB。摘要、位置及 JSON Pointer 指回未过滤报告。已完成检查的诊断进入 findings，缺证原因和诊断进入 execution_gaps，gate blockers 单独留存引用。 |
| B-03 / EVO-02-04 | 可选 `context` 保存原范围及最终 full 的复验 argv、已解析策略引用、基准、任务与工具预算。`--expect-base` 拒绝隐式基准漂移。 |
| B-04 / EVO-03-01–03 | 外部 Rust harness 调用同一 CLI，记录初始报告、修改、预算、复验及终止原因。反馈渲染无需调用模型。仅在显式执行闭环时运行所配置的外部命令。 |
| B-05 / EVO-03-04 | 覆盖策略修改、局部检查、快照失配及分支合并回归。对实际合并后的工作区重新运行 full，依据最终报告判定交付。 |
| B-06 / EVO-05 | 沿用已选 8 任务、7 天及各单元预算。连续 2 次无可验证进展即停止，所有失败留档。隔离仓库内执行模型调用、修改与构建，保护策略、测试及任务。真实模型、夹具、历史回放和收益分别标注。 |

反馈保留同指纹诊断的每个原始索引，任务关联只取 `plan.acceptance`。文本截断须标记，超预算的完整条目按类别记录总数、显示数及省略数。复验 argv 只能整条显示，空间不足时引用 context。调用者读取文件并校验摘要。完整报告生成前出错时沿用 incomplete 错误对象。可选受限适配器接收只读模型针对指定 `src/` 文件的严格 JSON 提案，控制器核对路径、旧内容与大小后写入并完整复验。[反馈与重放回归](../../tests/feedback.rs) · [外层循环](../../examples/agent_loop.rs) · [组合工作区回归](../../tests/agent_loop.rs) · [显式模型修复实验](../../tests/pilot_repair_live.rs)

### 阶段 C

**以独立案例校验规则演进，并据预先登记的运行单元汇总试点。** 来源元数据表达调用者声明，受保护的 suite/trust 固定引用。人工复核身份与未披露的外部调参历史仍须独立核实。

| 要求 | 交付与验收 |
| --- | --- |
| C-01 / EVO-04 | evidence 可记录案例来源、类别、复核人、调参用途、派生关系、预期拦截及正常行为。既有 archive 校验不可变来源摘要。 |
| C-02 / EVO-04 | 受保护 suite v2 的每个 case 指定 evidence_ref。held-out/anchor 引用有来源谱系、与生成/调参独立的样本，由候选作者以外的人类复核。缺引用、重复来源、循环或超预算谱系均拒绝。 |
| C-03 / EVO-04 | 对真实快照成对执行 baseline/candidate，使用独立 oracle 与既有签名推广/回退流程。反例覆盖生成样例通过但正常独立例被拦截，以及告警下降却漏检。旧 suite v1 保持原验证语义，v2 才核验独立案例的来源谱系，历史由 evidence、candidate、validation_attempt、evaluation 及批准记录保存。 |
| C-04 / EVO-05 | `pilot summarize --input` 有界读取版本化观察清单，核验完整报告长度、摘要和任务、策略、基准、快照绑定。按模型、工具、环境、缓存、来源、流程分组。 |
| C-05 / EVO-05 | 预登记 assignments 构成分母，列出未执行、失败、超时、放弃和排除理由。报告缺失或失配记为未完成。去重独立问题，显式报告未复核、未知模型、缺失成本与零分母。 |
| C-06 / EVO-05 | 汇总检测、误报、修复/首次成功、重复运行稳定性、必需检查完成、quick/full 耗时、人工时间及适用版本的成本，列明阈值与比较限制。真实试点验收和策略推广分别由其授权流程处理。 |
| C-07 / EVO-01-04 | 在修复与重构之外补充新功能、性能、依赖升级、文档/规则修改模板。各自声明前提、实际生产者与已知缺口，以解析和真实正反例核验。 |

领域层负责汇总，config 核验受限文件和报告，application 在 blocking worker 编排，interfaces 处理参数与呈现。模型沙箱接入、模块实验和生产观察分别分组。纯领域、CLI、受保护策略回归覆盖自批准、撤销、过期、受保护输入变更和回退历史。临时仓库的合成签名仅用于工程验证。真实独立样本、七天观察、人工复核及推广另行验收。[案例来源回归](../../tests/case_provenance.rs) · [汇总回归](../../tests/pilot_summary.rs) · [扩展模板回归](../../tests/pilot_templates.rs)

## 计划封存、授权与验收

### 阶段 D

**观察前封存计划**（EVO-05-01、EVO-05-02、EVO-05-04）。封存固定了将来要比较的输入和运行条件，团队把首次输出及摘要存入持久归档，作为七天观察的依据。

| 编号 | 契约及反例 |
| --- | --- |
| D-01、D-02 | `pilot seal --input` 对无 observation 的严格清单作有界读取，向 stdout 输出完整封存 JSON。已有观察、未知字段、输入超限或符号链接均拒绝。封存前须列明 owner、独立 reviewer、持久归档、封存及起止时间、样本数与周期。v1 还要求自由文本金额上限。 |
| D-03、D-04 | assignment 须有 real 来源与预先声明的 expected issues，禁止事后排除。每个 input 的所有运行单元共享 task、base、snapshot、任务摘要与真值身份，并具备相同的完整 cohort 矩阵。至少两个请求模型分别运行 existing-tools 和 qualitygate，缺组、重复或不平衡均拒绝。 |
| D-05、D-06 | SHA-256 绑定 schema、pilot ID、协议和全部 assignment，包括请求模型、Agent、harness、推理强度、流程、环境、工具、缓存与权限。运行后可补 `cohort.actual_model`、追加 observation，其他受保护字段变化使读取和汇总保持 incomplete。`pilot summarize` 显示 seal 或缺失限制。仅有 seal 的汇总为 `descriptive_only`、`requires_external_review`，身份、时间和归档真实性仍需外部证据。 |

封存流程在 blocking worker 中读取清单与时间，摘要由领域逻辑生成。D 阶段的纯契约覆盖幂等封存、模型后补、计划漂移、缺治理字段、缺真值与不完整矩阵。CLI 回归覆盖部分观察、篡改、缺归档和观察后封存。汇总另验报告摘要、长度、路径与缺失运行。[封存回归](../../tests/pilot_seal.rs) · [汇总回归](../../tests/pilot_summary.rs)

### 阶段 E

**认证启动授权**（EVO-05-01、EVO-05-04）。团队选定 human owner，由其在 CLI 外签发许可，reviewer 保持独立身份。

| 编号 | 契约及反例 |
| --- | --- |
| E-01、E-02 | `pilot authorization-subject --input` 从有效封存、尚无 observation 的清单导出唯一主题，绑定 repository、pilot ID、plan seal、owner、reviewer、seal/start/end、任务数和 assignment 数。缺 seal、已有观察或摘要漂移均不产生主题。 |
| E-03、E-04 | `pilot summarize` 同时接收仓库外 `--trust-store` 与 `--authorization` 后认证 DSSE/Ed25519 签名。human key ID 须等于 owner，允许该仓库并带 `pilot-plan-authorization` scope。签发须处于 seal 至 start 的窗口，有效期覆盖 end，当前时间、最大年龄及撤销检查均须通过。 |
| E-05、E-06 | 限制文件大小、类型与外部路径，发布前重读。错仓库、错主题、弱或重复 key、篡改、过期、撤销及读取期间变更均使认证失败。无授权时标记 `absent`、`protocol_ready=false`，有效授权标记 `authenticated`。普通签名能验证启动人对主题的声明，可信时间及最终接受仍须独立证据。 |

验签适配器先校验 DSSE PAE、签名与信任库，再解释 payload。单元回归覆盖晚签、有效期不足、撤销、Agent 自签与计划漂移，CLI 回归覆盖主题导出、认证/缺失状态和拒绝仓库内信任库。[验签测试](../../src/qualitygate/pilot_acceptance/authorization_tests.rs) · [授权集成测试](../../tests/pilot_authorization.rs)

### 阶段 F

**独立 reviewer 验收**（EVO-05-03、EVO-05-04）。以下九项阈值属于 v1 的历史口径。后续 manifest 的判据由其版本决定。

| 编号 | 契约及反例 |
| --- | --- |
| F-01、F-03 | `pilot acceptance-subject` 只在真实试点观察窗口结束、运行与报告完整、计划和启动授权有效后导出主题。v1 中实际模型未知也会阻断。主题绑定 repository、pilot、plan seal、完整 manifest 摘要、启动授权记录及签名 key、阈值评估详情摘要。改变 observation、报告引用、成本或人工数据均使旧签名失配。 |
| F-02 | `qualitygate-pilot-thresholds-v1` 合并全部 Qualitygate 组的检测、误报、修复和必需检查完成率的分子、分母及未知数，分母为零或含未知样本时保留 unknown。复核比例按已复核诊断数除以全部 distinct 诊断数。每个匹配对照分别检查 input inventory 一致、审查时间减少、full P95 比例及每项合格交付的成本比。任一对照失败使该项为 `not_met`，仅缺值为 `unknown`。评估绑定完整 groups/comparisons 摘要。 |
| F-04、F-05 | 独立 payload type 的验收记录只接受协议 reviewer 对应、带 `pilot-trial-acceptance` scope 的 human Ed25519 key。拒绝 owner 自批、错 key/仓库、过期或撤销。v1 九项均 `met` 方可签 `accepted`，完整证据允许签 `rejected`。v1 自由文本金额上限、归档权威、任务真值与收益解释由 reviewer 结合证据复核。 |
| F-06、F-07 | `pilot summarize --acceptance` 重验启动与验收记录。认证接受为 `authenticated_external_review/accepted`，认证拒绝为 `rejected` 并退出 1，无记录则为 `requires_external_review`。仓库外 trust store、启动和验收记录均须在发布前重读。签名 envelope、单文件与总输入受既有大小、类型和时间预算约束。 |

领域回归覆盖九项全部达标、失败、未知及两者并存。适配器和双模型×双流程、每单元两次尝试的 CLI 夹具检验提前签发、撤销、签名失配、接受与拒绝退出码。真实验收仍需正式身份、外部归档、八项真实任务及七天观察。[验收适配器测试](../../src/qualitygate/pilot_acceptance/acceptance_tests.rs) · [验收集成测试](../../tests/pilot_acceptance.rs)

### 阶段 G

**结构化总预算**（EVO-05-01、EVO-05-03）。manifest v2 首次把总额纳入可签署的验收主题，v1 保留原自由文本上限、封存摘要、九项阈值与人工预算判断。v2 的历史模板在阶段 H 演进为 v3。[v2 模板](../../templates/pilot/observation-v2.json) · [v3 模板](../../templates/pilot/observation-v3.json)

| 编号 | 契约及反例 |
| --- | --- |
| G-01、G-02 | 观察前封存 `protocol.budget` 中的 `currency`、`priced_at`、价格 `source`、`human_hourly_micros` 和 32 个运行单元共享的 `max_total_micros`。币种采用三个大写字母，一微币是该币种的百万分之一。v2 的旧字段 `monetary_cap` 须为空，缺项、零值、越界、币种错误或两套上限并存均拒绝。预算内容变化使原封存及授权失配，读取 v1 时保持原签名主题。 |
| G-03、G-04 | 用检查溢出的整数算术累计两模型、两流程的所有 assignment 和全部尝试，失败、超时、放弃均计模型与基础设施费。每条 observation 的人工费取 `ceil(review_active_ms × human_hourly_micros / 3_600_000)`。缺 observation、零尝试、缺成本分量或人工时间、币种或计价时间不一致均为 unknown。已知部分已超上限时为 `not_met`。 |
| G-05、G-06 | `qualitygate-pilot-thresholds-v2` 增加第十项 `total_cost_micros`，绑定总额与来源 manifest。超限或未知阻止签 `accepted`，仍可签 `rejected`。`pilot summarize` 列出三类已知费用与缺口数。既有 `cost_ratio` 仍按匹配组比较模型与基础设施成本。总预算另计全试点人工费用，直接与上限比较。 |

读取预算沿用 1 MiB manifest 与 64 MiB report 上限。领域和 CLI 回归验证 v1 摘要兼容、v2 预算漂移、失败成本、向上取整、已知超限与未知并存，以及成本比通过而全试点超额时拒绝接受。定价来源、正式预算和人工计时由团队提供。[预算计算](../../src/qualitygate/pilot_acceptance/pilot_contracts/budget.rs) · [验收集成测试](../../tests/pilot_acceptance.rs)

## 任务与执行证据的版本演进

### 阶段 H

v3 封存独立任务的分层配额（EVO-05-01、EVO-05-03）。

| 编号 | 契约与验收 |
|---|---|
| H-01 | 观察前声明 `protocol.task_mix`，每类配额为正整数，总和恰好等于 `task_count`。空配额、零值或溢出均拒绝封存。本轮模板为 4 项缺陷修复和 4 项重构。 |
| H-02 | 按不同 `input_id` 统计每类任务并核对配额。8 个输入在两模型、两流程下生成 32 个运行单元，改变任一输入的分层会导致核对失败。 |
| H-03 | 不同输入的 `task_id` 和 `task_digest` 各自唯一。同一输入在四个 cohort 中共享完整任务身份与真值。复用契约或只更换输入编号无法增加样本量。 |
| H-04 | 配额、任务 ID、类型、契约摘要及其余计划字段都受 seal 绑定。`sampling_audit` 列出声明和实际分层，每项输入只展示一次上述任务身份。manifest 上限为 1 MiB。 |
| H-05 | v3 沿用 v2 预算和十项验收阈值，v1/v2 保留原有序列化、摘要与验收语义。兼容回归要求同一 v2 输入在旧版与新版产生完全相同的封存 JSON 和摘要。 |
| H-06 | 团队与 reviewer 须用持久归档中的 issue、commit 及抽样时间复核任务独立性。两个不同摘要仍可能来自同一故障。 |

领域和 CLI 回归覆盖两类分层、缺失或错误配额、重复 ID 或摘要、封存后漂移及独立任务清单。[v3 模板](../../templates/pilot/observation-v3.json) · [封存集成测试](../../tests/pilot_seal.rs)

### 阶段 I

v4 为每个独立任务绑定一份可重读的来源工件（EVO-05-01、EVO-05-02、EVO-05-03）。

| 编号 | 契约与验收 |
|---|---|
| I-01 | `Manifest.sources` 为每个 `input_id` 声明一条 `TaskSource`，含 issue/commit 种类、`source_id`、归档相对路径、SHA-256、字节数和选入时间。多项、缺项、输入重复，或来源 ID、摘要、路径重复均拒绝封存。 |
| I-02 | 来源须为清单目录内的普通文件，排除符号链接和越界路径。文件非空，单份至多 64 KiB，总计至多 8 MiB。封存、启动授权主题、汇总和最终验收主题均重新核对长度和摘要。 |
| I-03 | `0 < selected_at <= sealed_at`。本轮的八个独立输入对应八份来源。时间由调用方声明，首次外部归档和可信时间须另行复核。 |
| I-04 | 来源清单及其全部字段进入 seal。事后改变 ID、摘要、路径或时间会使原 seal 和签名主题失配。汇总按输入展示来源种类、ID、摘要和选入时间。 |
| I-05 | v4 沿用预算、分层、十项阈值和签名语义。v1–v3 保留各自原有的 JSON 和计划摘要，旧版本拒绝新来源字段。兼容测试逐字节比较相同 v3 输入的封存 JSON。 |
| I-06 | owner 与独立 reviewer 对照外部 issue、Git 历史及持久归档确认来源内容与选入时间。哈希匹配只证明读取的文件与声明相符。 |

领域回归覆盖缺项、重复、时间与 seal 漂移。CLI 使用真实临时文件验证缺失、改动、符号链接、越界路径和汇总时复验。[来源校验](../../src/qualitygate/pilot_acceptance/pilot_contracts/validation.rs) · [封存集成测试](../../tests/pilot_seal.rs)

### 阶段 J

v5 将预定执行顺序与实际启动顺序分别纳入审计（EVO-05-01、EVO-05-03、EVO-05-04）。

| 编号 | 契约与验收 |
|---|---|
| J-01 | `run_order` 在观察前恰好列出每个 assignment ID 一次并进入 seal，缺项、重复和未知 ID 均拒绝。 |
| J-02 | 相邻单元交替执行 `existing_tools` 和 `qualitygate`。每个输入与请求模型都有一对流程。按任务类型和模型分层，两种流程各自先行的任务数之差至多为 1，层内四项任务须为 2:2。 |
| J-03 | observation 的 `start_sequence` 从 1 开始，在计划范围内且唯一。缺失表示顺序证据不足，偏离预定位置记作协议偏离。最终验收主题要求 `protocol_ready=true`。 |
| J-04 | `schedule_audit` 对照计划、已声明的实际启动次序，列出缺失和偏离项。计划顺序由 seal 绑定，观察序号由最终主题的 manifest 摘要绑定。reviewer 根据 harness 启动日志复核，结束时间不能推得启动次序。 |
| J-05 | v1–v4 拒绝非空计划顺序或观察序号，保留原封存字节。v5 沿用 v2 预算、v3 分层、v4 来源及十项阈值。 |

领域回归验证交替、平衡、序号唯一性、seal 漂移及旧版本拒绝新字段。CLI 验证偏离阻断和来源复验，旧版与新版封存同一 v4 输入应产生相同 JSON 字节。[顺序回归](../../src/qualitygate/pilot_acceptance/pilot_contracts/v5_tests.rs) · [封存集成测试](../../tests/pilot_seal.rs)

### 阶段 K

v6 用初始 full 报告重算每次修复的实际进展（EVO-03-03、EVO-05-01、EVO-05-03）。

| 编号 | 契约与验收 |
|---|---|
| K-01 | 每个 assignment 在观察前声明 `initial_report` 的相对路径、SHA-256 与字节数。协议声明 `0 < no_progress_limit <= max_attempts`。两者均进入 seal，v1–v5 保留原封存字节。 |
| K-02 | 封存、启动授权、汇总及最终主题均核对初始报告的受限路径、普通文件、长度与摘要、完整 full gate、初始快照及任务、策略、环境、工具、必需检查身份。缺失或损坏的报告无法充当基线。初始与尝试报告共用 64 MiB 读取预算。 |
| K-03 | 取完整已绑定 full 报告中的错误诊断 `check_id:fingerprint` 与 gate blocker 构成债务集合。核对由检查状态重算的 blocker，只有相对上一份已验证报告严格减少为真子集才算进展。换错、数量持平、仅改源码及缺失或不完整报告均不能重置无进展计数。保留报告摘要、前后债务数及判定。 |
| K-04 | 全部尝试计入次数、累计秒数、分母和成本。超过 `max_attempts`、`max_seconds` 或在连续 `no_progress_limit` 次无进展后继续尝试，均为协议偏离。失败、超时或放弃且无报告算作无可验证进展。声称已完成却无报告、引用损坏或基线缺失为证据不足。偏离或缺证令 `protocol_ready=false`，缺证还令 `complete=false`。 |
| K-05 | `attempt_audit` 逐单元列出基线、尝试、进展与预算、停止偏离，和汇总行共用报告身份校验。harness 与 reviewer 另核对计费、时间和 Agent 日志。旧版本拒绝新字段并使用原审计。 |

回归覆盖严格减少、换错、无报告、超次、超时、停后再试、初始报告漂移与越界，以及不同的已封存停止值。对相同 v5 输入比较旧版、新版完整封存 JSON 字节。[尝试审计](../../src/qualitygate/pilot_acceptance/pilot_contracts/attempt_audit.rs) · [v6 回归](../../src/qualitygate/pilot_acceptance/pilot_contracts/v6_tests.rs)

### 阶段 L

v7 为每个已观察运行单元保留一份模型采集记录（EVO-05-04）。

| 编号 | 契约与验收 |
|---|---|
| L-01 | seal 绑定观察前的请求模型、推理档位、Agent 与 harness 身份。观察产生的采集记录随观察归档。v1–v6 拒绝新字段并保留原序列化、摘要和验收语义。 |
| L-02 | manifest 声明 JSON 工件的相对路径、SHA-256、字节数与结构化内容。单份至多 64 KiB，合计至多 8 MiB。汇总及最终验收前核对受限路径、普通文件、长度、摘要和内容。缺失、变更、复用或身份不符均无法构成完整证据。 |
| L-03 | 记录绑定 assignment ID、Agent 版本、harness 摘要、请求模型、推理档位与采集时间，并与封存计划、观察时间及 assignment 的实际模型字段一致。`reported` 须给出模型标识，`unknown` 须给出原因。 |
| L-04 | 汇总逐单元列出请求模型、采集时间、报告的实际模型或未知原因、工件摘要与状态。比较按请求配置进行，明确记录未知模型本身允许协议就绪。模型路由、采集时钟与提供者身份由 reviewer 据外部原始日志复核。 |

领域回归覆盖请求与实际模型不符、未知、采集时间、缺记录和旧版本。CLI 以真实文件验证内容或摘要变更、越界与符号链接，兼容测试对照相同 v6 输入的完整封存字节。[v7 回归](../../src/qualitygate/pilot_acceptance/pilot_contracts/v7_tests.rs) · [工件读取测试](../../src/qualitygate/pilot_acceptance/inventory_loading_tests.rs)

### 阶段 M

v8 将模型证据细化到每次尝试，以发现重试时的路由漂移（EVO-05-04）。

| 编号 | 契约与验收 |
|---|---|
| M-01 | v8 拒绝运行单元级 `observation.model_evidence`。seal 仍绑定请求配置、Agent/harness 身份和既有计划。v1–v7 保持原封存和验收语义，拒绝非空的尝试级模型证据。 |
| M-02 | 成功、失败、超时、放弃和不完整尝试均可有一份 `attempt.model_evidence`。沿用 v7 的归档读取与 64 KiB / 8 MiB 限额，各尝试不得复用摘要。可读入缺记录的清单，汇总为 `complete=false`，最终接受主题不可生成。 |
| M-03 | 工件中的 `attempt_number`、assignment ID、请求配置及 Agent/harness 身份须与对应尝试和计划一致，采集时间在窗口内且不晚于观察。保留 `reported` 的具体模型或 `unknown` 的非空原因。只有全部尝试记录完整且报告相同实际模型时，才可填写 `cohort.actual_model`，其余情形须为 null。 |
| M-04 | 逐尝试展示状态、请求与实际模型、未知原因、采集时间和归档摘要，另列缺记录、未观察单元与路由漂移。单元内报告至少两个不同实际模型构成协议偏离并阻止最终接受。未知身份单列。不同的请求配置或环境分别分组。 |

回归覆盖失败和超时、缺档、未知身份、尝试号或计划失配、模型漂移及旧字段混用。CLI 验证真实文件的改动、越界和符号链接，并逐字节核对旧 v7 封存结果。[v8 领域回归](../../src/qualitygate/pilot_acceptance/pilot_contracts/v8_tests.rs) · [v8 集成回归](../../tests/pilot_seal/v8.rs)

### 阶段 N

v9 用逐尝试的 harness 执行记录交叉核对修复总时长与启动顺序（EVO-03、EVO-05）。

| 编号 | 契约与验收 |
|---|---|
| N-01 | 预算、身份和顺序仍在观察前封存。每个尝试可附一份 `execution_evidence`。v1–v8 拒绝新字段，保留各自封存 JSON 和汇总语义。 |
| N-02 | 严格 JSON 绑定 assignment ID、尝试号、开始与结束 Unix 毫秒、harness 摘要、状态、快照摘要及可空报告摘要，逐字段核对计划和观察。时差须等于 `elapsed_ms`，起止均在协议窗口内且不晚于观察。同单元已归档尝试的时段不能重叠，矛盾即拒绝。 |
| N-03 | 按受限相对路径、普通文件、SHA-256、精确字节数和严格 JSON 复核执行记录，单份至多 64 KiB，合计至多 8 MiB。seal、授权主题、汇总和最终主题输出前重读。可读入缺记录的清单，汇总为 `complete=false`，最终接受主题不可生成。 |
| N-04 | `execution_audit` 列出各尝试和缺失记录。按 `start_sequence` 排列各单元的首次尝试时，已归档开始时间须严格递增。相等或逆序令 `protocol_ready=false`，缺少首次记录则保持未完成。时间真实性仍须对照原始时钟与进程日志复核。 |

领域回归验证失败、超时、时差、重叠、状态及身份、报告失配。CLI 回归覆盖缺档、漂移、路径、符号链接、大小上限和逆序阻断，兼容性另核对 v1–v8 回归、v7 封存字节与发布包模板。[v9 领域回归](../../src/qualitygate/pilot_acceptance/pilot_contracts/v9_tests.rs) · [v9 集成回归](../../tests/pilot_seal/v9.rs)

### 阶段 O

v10 使用八项非财务验收检查（EVO-05）。本轮于 2026-09-17 确认取消定价要求。v9 建立的来源、顺序、初始报告、时间预算、停止规则、逐次模型与执行证据以及两次签名继续适用。

| 编号 | 契约与验收 |
| --- | --- |
| O-01 | 封存时 `monetary_cap` 为空，`budget` 为空或省略，`thresholds.cost_ratio_max` 无值。v10 拒绝非空旧金额控制，v1–v9 保留各自封存的金额约束和签名语义。 |
| O-02 | 可记录实际用量与有来源的费用。缺少定价时，金额保持未知，质量、完成率、审查和耗时仍按各自证据计算。 |
| O-03 | 最终主题使用 `qualitygate-pilot-thresholds-v3`。本轮要求检测率 ≥90%、误报率 ≤5%、修复率 ≥80%、必需检查完成率 ≥95%、人工复核比例 100%、各对照输入清单匹配、审查时间减少 ≥10%、full 检查 P95 耗时 ≤基线的 1.2 倍。已知未达标优先判失败，缺必需测量保持未知，八项全为 `met` 方可签接受。 |
| O-04 | 仓库与 Skill 的 v10 模板字节一致。旧模板可用于相应版本的重放，打包检查覆盖当前模板和引用。 |

v10 领域测试覆盖无定价封存、拒绝混入旧金额控制、缺模型记录及八项阈值的通过、失败和未知。CLI 集成测试验证封存结果，打包测试核对 Skill 模板。工程验证分别采用单元与集成测试、文档与架构检查、完整 selfcheck、Cargo 与 Bazel 构建、Clippy 及至少 90% Rust 行覆盖率。Miri 和 ASan 分开运行。真实试点的退出证据须按上文收尾条件由团队提供。[v10 领域回归](../../src/qualitygate/pilot_acceptance/pilot_contracts/v10_tests.rs) · [验收算法](../../src/qualitygate/pilot_acceptance/pilot_contracts/acceptance.rs) · [v10 CLI 测试](../../tests/pilot_seal/v10.rs) · [Skill 包校验](../../tests/quality/skill_package.rs)
