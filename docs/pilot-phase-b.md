# 阶段 B：任务与反馈闭环记录

> 日期：2026-09-16；需求：EVO-01–03；收益观察沿用 8 个任务 / 7 天。

## 1. 交付范围

按 [SDD 设计](../codespec/design/pilot-phase-b.md)新增
[有界 Agent 反馈](agent-feedback.md)和[外层 Rust 修复循环](agent-loop.md)。
沿用阶段 A 的 Rust 缺陷修复、重构模板；完整报告与精简视图使用同一门禁结论。
CLI 内部不调用模型。调用方选择 Agent、任务和策略；Rust harness 固定输入、记录尝试、
保存改动和证据，在无进展、未完成或预算耗尽时停止，并执行最终完整复验。

用户确认的 Codex Terra/Luna 两组继续适用，推理强度均为 medium。
这验证同一产品的跨模型接入；跨产品兼容性仍待观察。

## 2. 受控实际模型实验

实验源自本仓库 `src/qualitygate/domain/ratchet.rs`，提取到隔离的最小 Cargo 仓库，
保留原始源文件摘要。两个任务共用预先写定的三个独立回归用例：

- 缺陷修复：注入 `>` 变为 `>=` 的故障，要求相同数量不算债务增长；旧实现有真实断言失败。
- 重构：改用 `BTreeMap<Key, Measurement>` 累积测量项，保持排序、缺省值、计数和独立桶语义；原实现已通过。

测试、Cargo 输入和任务文件属于受保护资产。每种模型使用独立副本和会话、相同任务提示、
最多 3 次尝试 / 30 分钟及连续 2 次无进展终止条件。记录每次完整报告、模型输出、补丁和
用量。接受条件包括代码快照实际改变、原任务/策略不变、full 范围全部完成并通过。
本实验没有替代八个独立生产任务，也没有测量人工复核时间或完整金额成本。

### 环境失败与接入修正

Codex 0.154.0 的本地 workspace-write 配置与 legacy Landlock 后端不兼容，
工具返回 `permission profiles requiring direct runtime enforcement are incompatible with --use-legacy-landlock`。
初始实验保留在 `target/pilot-phase-b/repairs-P9PKaY`；其失败和用量不能从记录中删除。

接入改用 read-only 模型与严格 JSON 源文件提案。模型读取代码和报告，返回完整替换内容；
控制器仅能替换明确指定的 `src/ratchet.rs`，之后独立执行相同契约。
未扩大模型的文件权限，也未关闭沙箱。该模式的限制和路径/输入保护见外层循环文档。

### 2026-09-16 实际结果

修正后的记录位于 `target/pilot-phase-b/repairs-aexp2m/index.json`，摘要
`sha256:67359de7c5b584557ab8dde9a48a09af834d54adb10ae100f1836a1a6605422f`。

| 请求模型 | 任务 | 初始门禁 → 最终 full | 尝试 | 模型进程耗时 |
|---|---|---|---|---|
| gpt-5.6-terra | 缺陷修复 | fail → pass | 1 | 40.459 s |
| gpt-5.6-terra | 重构 | pass → pass，源码已改变 | 1 | 33.369 s |
| gpt-5.6-luna | 缺陷修复 | fail → pass | 1 | 39.350 s |
| gpt-5.6-luna | 重构 | pass → pass，源码已改变 | 1 | 44.872 s |

四次最终复验均执行三个独立回归用例，保留任务、策略、基准和完整快照。
实验构建由报告中的 evaluator 摘要与 manifest 中的 harness 摘要标识；成功组
保留摘要匹配的 `harness-source.rs`。随后新增的基准保护与错误输出由离线门禁验证，
未重跑收费模型；实测记录对应其明确的开发构建。
补丁检查确认两次修复恢复严格 `>`，两次重构确实改为有序映射累积。
耗时仅为模型进程墙钟时间，不代表人工活跃分钟或服务端纯推理时间。
四次 Codex 调用合计报告 input_tokens 158,313、output_tokens 4,806、
cached_input_tokens 90,624；缓存数属于输入量的子集，不能再次相加。
实际服务端模型版本与金额成本仍未知。

初始 workspace-write 实验的四个运行单元全部因无进展停止，各保留两次调用，
共 input_tokens 1,458,337、output_tokens 12,575、cached_input_tokens 1,237,760。
初始 manifest 摘要为
`sha256:f36d7b1347bf89f40fc81d24a556c7eea6fb753769f1209b76079c12d5530ff3`。
两种接入配置共 12 次 Codex 调用，不等于 12 次底层 API 请求。
失败配置与成功配置分别分组；不能只保留后者来计算试点修复率或成本。

成功组八份初始/最终完整报告共 95,255 字节，精简输出共 27,948 字节，
本组输出字节减少约 70.7%。没有运行同一模型的 full 反馈对照，不能宣称 token、
审查时间或完整交付成本降低，更不能用四个相关小样本给模型能力排序。

## 3. 需求到测试

| 需求 | 实现与执行证据 |
|---|---|
| B-01 / EVO-01 | `templates/pilot/rust-v1`；`pilot_baseline` 的真实断言、旧失败/新通过、重构双通过、零测试与编译错误 |
| B-02 / EVO-02-01–03 | `domain::feedback` 单元：重复指纹、UTF-8 截断、字节预算、未知关联、违规/未完成并存；`feedback` 集成验证完整报告摘要和过滤一致性 |
| B-03 / EVO-02-04 | `feedback` 集成：移动策略分支后执行固定复验；path 变为完整任务验收；staged 基准移动拒绝复用；修改任务不能消除原条件 |
| B-04 / EVO-03-01–03 | `examples/agent_loop.rs` 与 `agent_loop` 集成：成功、无进展、次数耗尽、Agent 失败、未完成输入、未跟踪改动留存；提案路径/旧输入/体积保护 |
| B-05 / EVO-03-04 | `pilot_baseline::separately_passing_branches_require_a_new_full_check_after_merge`；既有 `execution`、`policy` 保留执行期间变化与 staged/worktree 失配反例 |
| B-06 / 实际模型接入 | 显式运行 `pilot_repair_live`，普通 gate 忽略收费调用；原始失败与修正后的记录分别保留 |

## 4. 阶段退出与剩余观察

阶段 B 已获得两种模型实际源文件改动和最终完整验收记录；工程质量门禁另记如下，业务收益仍依 EVO-05 单独验收。
[阶段 A](pilot-phase-a.md)已确认的 8 个任务、7 天、预算和阈值保持不变。
具体生产任务清单、复核人、金额上限与持久归档尚未封存；七天观察并未由上述实验完成。
人工收益、完整成本、检测率和误报率保持未知，不使用本次四个受控运行单位充当业务分母。

## 5. 工程质量验收

本地证据目录为 `target/pilot-phase-b-verification`。本轮实现基于
`971455bd705c35e7ba7a7ea5fa905b90eaae2a54` 后的未提交工作区，不代表发布版本。

| 门禁 | 结果与证据 |
|---|---|
| 格式、全部目标/特性编译、Clippy `-D warnings` | 通过；`fmt.log`、`check.log`、`clippy.log` |
| `cargo test --locked --all-targets --all-features` | 388 通过，12 个需显式运行的测试忽略；`all-tests-final.log` |
| 单元与集成分项 | 原生库单元 201；集成目标合计 184；范例单元 3；不能把重复 helper 执行计为独立业务样本 |
| `cargo llvm-cov --locked --all-targets --all-features --fail-under-lines 90` | 通过，20,254 行中未覆盖 862，行覆盖率 95.74%；`coverage.log` |
| 文档、架构与 Skill 包 | 14 项通过；架构 146 份源摘要、1,425 条引用、0 违规；`quality.log` 及 `target/architecture/report.json` / `graph.dot` |
| CodeSpec / Knowledge 地图 | 两份结构校验通过；`maps.json` |
| 完整 selfcheck | 338 fixture 全部完成并与 golden 一致；`selfcheck.json` |
| Bazel | `//:qualitygate` 构建和 `bazel test --lockfile_mode=error //...` 11 个目标通过 |
| 外层循环收尾验证 | 最终 7 项集成/helper 测试通过；无效输入实测返回 incomplete 摘要与退出码 2；`harness-integration-final.log`、`harness-invalid-input.json` |
| 实际模型 | 初始写入接入测试失败并留档；修正后的只读提案接入测试通过，四个任务运行单位均有新快照及 full pass；`live-repairs.log`、`live-proposals.log` |

Miri、ASan 和其它外部生产者保留各自独立 gate，本轮未重新运行，不能从稳定套件结果推断
它们通过。模型记录的 evaluator/harness 摘要界定当时开发构建；后续有针对性的补充检查
覆盖当前控制器的 I/O 隔离、错误返回和基准保护。未上传证据、发布版本或替代正式试点验收。
