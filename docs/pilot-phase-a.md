# 阶段 A：可比较基线记录

> 创建日期：2026-09-15；参数确认：2026-09-16；记录版本：v2
> 状态：阶段 A 工程准备已完成，试点规模、周期及阈值已确认；任务清单等执行输入待封存，尚无业务收益结论
> 依据：[演进 SPEC](../codespec/requirements/agent-acceptance-evolution.md)、
> [阶段执行设计](../codespec/design/pilot-phase-a.md)、[试点协议](pilot.md)

## 1. 已确认范围与模型选择

用户明确要求：本地只使用 Codex 的不同模型模拟两种 Agent，模型能力区分中、低水平。
本轮据此固定同一 harness 下的两个模型配置组；产品种类数仍是 1。

| 配置 | 中等能力组 | 较低能力组 |
|---|---|---|
| 请求模型 | `gpt-5.6-terra` | `gpt-5.6-luna` |
| 选择依据 | 官方定位为智能与成本平衡、约对应此前 mini 层级 | 官方定位为高吞吐与成本敏感、约对应此前 nano 层级 |
| 推理强度 | `medium` | `medium` |
| harness | Codex CLI `0.154.0` | Codex CLI `0.154.0` |
| 会话与输入 | 独立克隆、无历史会话、同一提示词与任务 | 同左 |
| 首轮接入探测 | 每配置 1 次，180 秒，每输出流最多 16 MiB | 同左 |

选择依据来自研究当日核验的 [Terra 官方模型文档](https://developers.openai.com/api/docs/models/gpt-5.6-terra)
和 [Luna 官方模型文档](https://developers.openai.com/api/docs/models/gpt-5.6-luna)。
“中、低”是本轮相对层级标签，实际任务能力仍需测量；不用推理强度差异同时改变第二个变量。
本地模型缓存列出了两个模型，实际请求和执行结果见下文；缓存不能证明账号调用成功。
JSONL 未提供可认证的服务端实际模型身份，记录 `actual_model: null`，不把请求名称当作证明。

## 2. 预注册参数

阶段 A 的工程准备使用本仓库；固定源码提交为 `971455bd705c35e7ba7a7ea5fa905b90eaae2a54`。
当前新增 SPEC、模板、测试和 CI 接线属于工作区变更，不属于该源码提交。
模型接入用该提交的隔离克隆；工具基线明确记录所选策略、快照与检查范围。
受控 Cargo 样例独立标注为 fixture，不混入真实任务样本。

2026-09-16 用户确认：“缩小为 8 个任务、7 天，其他参数相同”。
据此采用 qualitygate-cli、缺陷修复/保持行为的重构各 4 个，保留既定模型组、比较方法、
单次预算和接受阈值。此确认替代 v1 的规模与周期建议，尚未开始真实任务收益观察。
7 天周期从任务清单等执行输入封存后的实际启动时点起算，另行记录开始与结束时间。
负责人、具体任务、金额上限和持久归档位置仍按下表补齐，不把未提供的值视为已确认。

| 输入 | 已确认参数与执行口径；未提供项明确标记 |
|---|---|
| 项目与来源 | 当前 qualitygate-cli；历史回放使用固定提交的隔离克隆 |
| 负责人 | 由本次需求提出者指定试点复核人；不由模型生成审批身份 |
| 样本与周期 | 8 个独立任务，缺陷修复/保持行为的重构各 4 个，7 天 |
| 采样 | 观察前封存任务清单；同一任务分配给两模型并分别采用既有流程/接入流程；8 个任务共 32 个任务运行单元 |
| 比较方法 | 两种模型 × 两种流程；按任务类型分层、交替顺序；固定工具/缓存条件，模型差异和 CLI 差异分别比较 |
| 修复预算 | 每运行单元最多 3 次尝试、总计 30 分钟；连续 2 次无可验证进展终止；最多 96 次尝试/16 小时累计运行时间 |
| 复核 | 新增独立问题与全部阻塞诊断 100% 复核；记录未复核量、活跃人工分钟、排除理由 |
| 策略与任务 | 沿用受信流程固定策略提交；任务采用模板后须审查具体行为、锁定契约摘要和验证资产 |
| 证据位置 | 当前工程证据在 `target/pilot-phase-a`；真实试点开始前由负责人指定持久外部归档，禁止提交凭据 |
| 金额预算 | 本轮接入探测使用现有 Codex 配额；未取得账单价格和团队金额上限，金额为未知，不启动批量收益采样 |

| 指标 | 已确认接受阈值 | 判定前提 |
|---|---|---|
| 已知问题检测 | 至少 90% | 独立复核的问题集；没有已知问题分母时未知 |
| 误报率 | 已复核诊断中不超过 5% | 同时披露未复核量和既有工具归因 |
| 修复成功率 | 预算内至少 80% 完整复验通过 | 全部已分配的有效可修复案例进入分母 |
| 必需检查完成率 | 至少 95% | 完成且失败、跳过、未完成分别统计 |
| 人工审查负担 | 活跃分钟中位数相对基线减少至少 10% | 分任务类型和模型比较，完整记录各组 |
| 验证开销 | 同范围 full 耗时 p95 不超过基线的 1.2 倍 | 工具、机器、缓存条件相同；额外检查单列 |
| 合格交付成本 | 完整成本口径下不高于基线 | 失败尝试计入成本；人工计价、币种、价格时间与覆盖缺失时未知 |

计数、分母与原始记录同时提供；小样本不能支持普遍性结论。观察后的参数变化必须另起版本和比较组。
缩小样本不降低阈值；按实际计数与分母比较，不能通过四舍五入放宽接受条件。
连接探测和模板 fixture 不进入上述分母，也不产生准确率、修复率、时间节省或金额收益结论。

## 3. 已实现的工程资产

- [Rust 缺陷修复模板](../templates/pilot/rust-v1/bug-fix.yaml)和
  [重构模板](../templates/pilot/rust-v1/refactor.yaml)，均使用现有任务契约。
- `tests/pilot_baseline.rs`：临时 Cargo 项目执行真实断言，旧缺陷失败/修复后通过；
  重构前后通过；零测试阻塞、编译失败未完成、修改所选任务不能删除验收要求。
- 受控报告保存原始 stdout/stderr、工具探测日志、摘要、task/policy/snapshot 身份与路径迁移索引。
  报告保留原始路径，归档后的日志位置从 `index.json` 查找，不伪造原始执行路径。
- `tests/pilot_codex_live.rs`：显式启用的真实模型接入探测，共用既有 Rust 有界 runner。
  保存初始失败、请求参数、回答、事件、用量、时限和终止状态，不自动重试或替换模型。
- 本地 integration 和 PR integration 门禁纳入模板回归，`templates/**` 纳入验证资产。
  模型测试默认忽略，常规测试和覆盖率不会消耗模型配额。

模板只检查单次运行；两份报告由受控回归建立对照。普通 Cargo 汇总不提供逐文件断言分类，
真实缺陷反例还需复核日志；自动逐文件证明需显式配置兼容的 `test_effectiveness` 生产者。
重构不强制旧代码失败，未声明行为和 API 等价性仍属于模板边界。

## 4. 实际探测与环境缺口

首轮默认只读沙箱下，两个模型均返回响应，但没有成功的 CLI 命令执行证据。
模型回答报告 `bwrap: loopback: Failed RTM_NEWADDR: Operation not permitted`；随后本地
`codex sandbox` 直接执行版本命令复现了相同错误，确认这是环境问题。
首轮证据目录为 `target/pilot-phase-a/codex-iMKaNA`，测试失败如实保留。

Codex 当前支持显式 `use_legacy_landlock` 后端。本地使用相同只读策略直接运行 CLI 成功，
同时向临时目录写文件被拒绝；日志在 `target/pilot-phase-a-verification/sandbox-*.log`。
该选项已被 Codex 标记弃用，不能承诺未来兼容；本轮不修改用户全局配置。
两组采用相同后端重新建立配置组，不能把不同后端结果配成模型能力比较。

另一次 Landlock 准备检查在调用模型前发现 debug 可执行文件超过 256 MiB 身份预算，
结果保留在 `codex-09Sj8o`。正式复探显式选择当前源码构建的 release CLI，并保持身份预算。
服务端成本和模型身份未知、工具调用事件缺失、环境受限均保留为缺口，不能用模型自述补齐。

最终保留记录 `codex-333DAb` 的两个模型均有成功版本命令和正确契约回答。
探测脚本曾把裸版本 `0.4.0` 和包含其它命令输出的版本日志误判为缺失；
现已支持版本/横幅两种回答以及独立版本输出行，并用失败命令、无关命令和猜测版本作为负例。
早期回答路径的 OS 编码也显式兼容读取，归档仍要求单一文件名、字节数与摘要一致。
修正后离线重审原始事件与回答，两个模型均通过；没有再次调用模型，也没有覆盖原始失败结论。

| 最终配置组 | Codex 进程耗时 | 输入 / 输出 token | 已缓存输入 token | 证据复核 |
|---|---|---|---|---|
| Terra / medium / Landlock | 40,276 ms | 72,349 / 935 | 63,488 | 实际版本命令、契约回答、工作副本未改动均通过 |
| Luna / medium / Landlock | 31,745 ms | 48,843 / 922 | 26,112 | 同上；原始脚本格式误判保留，离线重审通过 |

上述是单次接入观察，不能据此排名或推导修复效率。所有准备轮次合计 6 次 Codex 探测运行，
每次运行可能包含多次模型请求；
包括失败和修正前的探测：输入 313,087、输出 4,932、已缓存输入 194,304 token。
已缓存输入是输入的子集，不再相加；这些是 Codex 返回的用量，金额仍为未知。

| 证据文件（相对仓库） | SHA-256 |
|---|---|
| `target/pilot-phase-a/codex-333DAb/index.json` | `7a772b4cfe23e9eead0c1c3752253728f09d5d0f04a9b4b59a6b31b3c51b4044` |
| `target/pilot-phase-a/codex-333DAb/reassessment.json` | `1f77439e414160f329f186e2cf32edbeba54f3074622cb27689f0d305ad53486` |
| `target/pilot-phase-a/repository-baseline.json` | `73b59b9aa658d53f3ba0034c1f05fedc183971132e5c2933a7eb5fbb2ce6b6fc` |

仓库基线固定源码和策略于上述提交：`line-ending`、`format` 均完成通过；
这是 `quick` 仓库范围，报告明确保留 9 个待执行检查，不构成任务 full 验收。
本轮工作区的完整 Rust 工程检查另记在 `target/pilot-phase-a-verification`，不得混成同一快照报告。

### 重跑命令

离线工程回归：

```bash
cargo test --locked --all-features --test pilot_baseline -- --test-threads=1 --nocapture
```

以下命令需要本机 Codex 登录态并实际消耗配额，不在 CI 默认运行。
仅当本机需要且仍支持该后端时，显式选择 Landlock；其它环境省略该变量使用默认后端。

```bash
cargo build --locked --release
QUALITYGATE_PILOT_CLI=target/release/qualitygate QUALITYGATE_PILOT_SANDBOX_BACKEND=landlock cargo test --locked --all-features --test pilot_codex_live codex_medium_and_lower_models_share_the_same_cli_contract -- --exact --ignored --nocapture
```

离线重审已有记录不调用模型；它校验文件摘要，生成带原始索引和验证器摘要的独立 `reassessment.json`：

```bash
QUALITYGATE_PILOT_REPLAY=target/pilot-phase-a/codex-333DAb cargo test --locked --all-features --test pilot_codex_live recheck_recorded_codex_probe_artifacts -- --exact --ignored --nocapture
```

每次运行使用新证据目录；保留失败目录，不用最终成功覆盖最初观察。
模型接入探测只检查版本命令与既有契约理解，实际诊断修复/最终 full 验收在阶段 B 另行观察。

## 5. 需求到测试及阶段验收

| 要求 | 工程证据 | 验收边界 |
|---|---|---|
| A-01 / EVO-05-01 | 本文模型组与 2026-09-16 参数确认记录 | 项目、模型组、8 个任务/7 天、单次预算及阈值已确认；具体任务清单、责任人、金额上限和持久归档仍需封存 |
| A-02 / EVO-01-01–03 | `task_templates_capture_real_assertions_and_allow_behavior_preserving_refactors` | 模板及真实断言的受控对照，不代表业务样本 |
| A-02 / EVO-01、03 | `task_preparation_rejects_zero_tests_compile_errors_and_changed_acceptance` | 沿用现有门禁，失败和未完成不能通过 |
| A-03 / EVO-05-02 | 固定源码的 CLI 工具报告及本轮 Rust 质量日志 | quick 与完整工程检查分开，工作区新增资产不冒充既有提交内容 |
| A-04 / EVO-03-01 | `codex_medium_and_lower_models_share_the_same_cli_contract` | 真实响应、命令证据与缺口分开；同产品两模型不证明跨产品兼容 |
| A-04 / 证据解释 | `version_answers_accept_the_version_or_banner_and_reject_guesses`、`command_observations_accept_a_banner_line_but_reject_failed_or_unrelated_commands`、`recheck_recorded_codex_probe_artifacts` | 版本格式正反例与摘要校验后的离线重审，不用模型自述代替命令证据 |
| A-04 / 资源边界 | `probe_artifact_reads_reject_missing_and_oversized_files` | 缺失回答和超过 16 MiB 的文件拒绝读取为有效证据 |
| A-05 / EVO-05-03–04 | 本文执行记录及能力缺口 | 工程验证、阶段输入确认、真实收益验收分别记录；不改变 M1–M4 状态 |

当前没有真实修复/审查时间/金额收益样本；不能把工程通过标为真实试点通过。
图索引仍停留在旧提交，增量任务因 fixture 事实大小限制重试；本轮直接核对当前源码，地图验证另行保留。

### 本轮工程检查结果

所有日志位于 `target/pilot-phase-a-verification`。以下测试执行有重叠，不累加为独立样本：

| 检查 | 结果与证据 |
|---|---|
| 格式、所有目标/特性编译、Clippy `-D warnings` | 通过；`fmt.log`、`check.log`、`clippy.log` |
| `cargo test --all-targets --all-features` | 364 通过、0 失败、10 忽略；其中领域/核心原生单元 198，通过日志与各集成目标分别记录于 `tests.log` |
| 最终探测脚本的离线回归 | 3 通过、2 显式忽略；`probe-contracts.log`，包含最后补充的文件大小边界 |
| 全目标/特性覆盖率重跑 | 365 通过、0 失败、11 忽略，Rust 行覆盖率 **95.75%**，高于 90%；`coverage.log` |
| 完整 selfcheck | **338/338**，`complete: true`、`decision: pass`；`selfcheck.json` |
| 文档、架构、地图 | 通过；`documentation.log`、全量 quality 目标、`target/architecture/report.json` 和 `maps.json` |
| 真实模型记录离线重审 | 两组通过；`codex-replay.log`、原始索引及独立 `reassessment.json`；最初 live 脚本失败未改写 |

本轮未运行独立的 Miri/ASan nightly 门禁。被忽略的外部生产者和模型探测不计为普通测试通过；
模型运行及重审已有上文的单独证据。既有质量标准、失败门禁、golden 和生产 CLI 判定均未降低。
