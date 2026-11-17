# 可扩展工具事实与规则断言设计

> 文档版本：v0.2
> 日期：2026-11-17
> 状态：首期已实现；未声明生产采用或二进制发行
> 行为规范：[工具事实与规则断言需求](../requirements/tool-facts-and-assertions.md)
> 评审基线：`900c795b893da5af472662bacef04cac82c0b270`

## 1. 设计范围与已有基础

本设计细化 TFA-01–TFA-08 的实现边界；行为、数值、完整性和验收条件以需求文档为准。
首期接口已进入 Rust 开发实现；以下概念示例不替代导出的闭合 Schema。
工具输出配置与范围限制见[工具报告参考](../../docs/en/02-reference/03-projects-and-tool-reports.md)，
规则语法见[V4 参考](../../skills/qualitygate-cli/references/operators-v4.md)。

现有架构已提供不可变快照、工具与产物身份、有界 runner、外部报告归一化、有限规则 DSL、
依赖计划及策略采用流程。扩展复用这些边界，不新增第二套执行调度器或门禁裁决器。

| 已有实现 | 本设计需要处理的变化点 |
| --- | --- |
| `check_planning::model::RuleSource` 选择源码事实 | 增加显式选择生产者输出的 source，保留源码 source 的原有语义 |
| V3 的实体字段为名称、文本和路径 | V4 根据事实类型检查字段和比较类型，不改变 V3 的字段解释 |
| `report_evidence::tool_reports::Data` 归一化诊断、测试和覆盖率 | 新增工具事实输出分支，不把指标或关系塞进诊断文本 |
| `gate_execution::rule_execution` 从报告 metadata 读取项目事实 | 以类型化运行结果传递事实，报告 JSON 作为兼容投影 |
| workflow 按 command/manual/compatibility/test_effectiveness 分派 | 规划阶段归一化执行节点，执行阶段保留共同依赖和完成时复核 |
| Bazel 按 capability 独立编译，Cargo 为一个 package | 首期新增可独立构建的接入程序，保留现有包与 CLI 标签 |

## 2. 能力归属与依赖方向

沿用[开发视图](../../docs/en/03-architecture/03-development-view.md)的 capability contract。
实现放在 `src/qualitygate` 下直接命名的业务 concern；不在兼容 facade 下新增实现层。
`tool_evidence` 与 `tool_bridge` 是业务 concern；一个 concern 内的具体模块按其实际能力登记。

| Owner | 实现职责 | 禁止承担的职责 |
| --- | --- | --- |
| `domain` | 工具事实、类型及版本描述，纯数值比较、完整性判断与规则结果契约 | 文件读取、进程句柄、AST 缓存、CLI 参数 |
| `config` | 工具输出声明、V4 source 与字段验证，依赖及执行计划编译 | 调用工具、推测实际采集完成 |
| `snapshot` | 复用选定快照、物化输入、源路径与内容身份验证 | 解释工具指标或业务阈值 |
| `adapters` | 有界解析、格式归一化、能力描述及原始结果语义映射 | 改写项目策略或代替 Gate 决定交付 |
| `runner` | 外部工具的命令、超时、输出及取消边界 | 将任意非零退出解释成已发现缺陷 |
| `application` | 生产者执行、事实存储、依赖消费、完成时复核与聚合 | 解析上游 EDN 格式或逐工具重写阈值规则 |
| `interfaces` | 桥接入口、帮助、Schema/能力展示和输出渲染 | 直接访问文件或执行子进程 |
| `env` / `paths` / `net` | 继续拥有各自既定的外部能力 | 为适配器提供无边界的环境、路径或下载访问 |

纯事实模块进入 reviewed owner map 和 Miri inventory；适配器继续不能依赖 CLI。
公共历史 facade 保持兼容。新的归一化代码以 Rust 库提供，CLI 接入程序调用相同实现。
domain 只持有纯能力描述；具体 parser 的静态注册及函数分派由 adapters 持有。
描述与实现通过注册一致性契约验证，不通过反向依赖拼成全局插件对象。

## 3. 运行数据流

```mermaid
flowchart LR
    P[选定策略与快照] --> C[配置验证与执行计划]
    C --> R[现有 Runner 执行外部工具]
    R --> A[Rust 适配器校验并归一化]
    A --> F[类型化事实存储]
    F --> E[V4 source 与断言]
    E --> V[完成时身份与证据复核]
    V --> G[现有 Gate 裁决]
    F --> O[报告兼容投影]
```

### 3.1 计划与执行结果

通过配置验证的计划在分派前归一化为 `PlannedOperation`，表达已有规则、命令、手工验收、兼容验证和
成对测试执行方式。事实采集属于命令的输出契约，不为 crapper 或 dryer 新增执行器种类。
计划节点统一携带 ID、required、severity、depends_on、范围及预算；不改变现有 YAML 的默认解释。

内部结果为 `ExecutionOutcome { check_result, evidence }`，类型化 `evidence` 分别持有项目事实、
工具事实、已验证覆盖率及真实执行凭据。
`check_result` 继续满足现有执行状态与裁决模型，工具事实是经校验的类型化集合。
采集失败时可保留原始产物和部分解析证据用于诊断，但不能发布可被成功断言消费的完整集合。
普通 lint command 继续按现有诊断与退出码规则裁决；事实生产者必须显式声明自己的输出契约。

应用层按生产者节点保存结果，只将已声明依赖且满足采集契约的集合提供给消费者。
同一运行内，一个生产者的相同范围输出可以被多个 source 引用；不因规则数增加而重复执行工具。
首期不引入跨运行事实缓存，也不因本次拆分增加共享 workspace 上的并发命令。

### 3.2 身份与完整性

`ToolFacts` 信封由统一协议版本、输出契约、采集范围、记录及证据引用组成。
采集范围包括声明的项目/target inventory、配置身份、采集筛选条件和确认完成的范围。
记录类型、数值约定及空集合条件由 TFA-02、TFA-03、TFA-05 定义。

工具和独立桥接程序提交的 snapshot/producer 字段只是待校验声明。应用层将其与选定快照、
实际命令、工具版本和保留产物逐项绑定，不能仅因外部 JSON 写了正确的摘要就认定受信执行。
从 stdin 或磁盘导入的报告沿用相同验证要求；离线格式转换成功不证明原分析已在目标快照运行。

完成时仍执行现有输入、策略及外部证据复核，再计算约束覆盖和最终 Gate。
失效的生产者事实及依赖其产生的断言结果不能保留可接受结论；报告保留失效原因和原始运行记录。
既有报告 metadata 的兼容输出不再是内部事实权威，消费端不反向解析展示 JSON 获取事实。

## 4. 接口与版本策略

### 4.1 工具输出与规则 source

接口分为三个层次，版本各自显式记录，不将上游工具版本当成协议版本：

| 接口 | 提供方与消费者 | 首期内容 |
| --- | --- | --- |
| 工具事实协议 v1 | 适配器/桥接程序 → Qualitygate | 函数指标和重复代码对，采集范围、完整性及证据引用 |
| 项目规则 schema v4 | 策略作者 → config 与规则引擎 | 源码或工具事实 source、类型化字段、既有有限组合算子 |
| 运行能力声明 | CLI → 策略验证及调用方 | `rules.operators.v4`、事实 schema 和适配器支持范围 |

工具 source 的逻辑接口是“生产者 check ID + 输出/事实 schema + 项目与 target 选择条件”。
config 在执行前验证生产者存在、依赖显式声明、输出兼容及字段类型；未知字段或单位不匹配
属于规则无效，运行中缺少契约要求的字段属于证据未完成。两种错误均不能变成普通断言为 false。

下面是语义伪码；可复制的 YAML 示例见 V4 参考，具体语法由导出 Schema 约束：

```text
source functions := facts(producer = complexity_scan, type = function_metric)
source duplicates := facts(producer = clone_scan, type = clone_pair)
assert count(selected(functions)) >= policy.minimum_functions
assert all(selected(functions), metric("cyclomatic_complexity") <= policy.limit)
assert count(filter(duplicates, similarity >= policy.similarity_limit)) == 0
```

指标名称与测量定义分离：采用相同已注册事实类型的新工具可以提供新指标名称，规则明确选择
该名称、单位和测量定义；不需要让 Gate 增加工具 ID 分支。新的事实结构需要显式版本和
能力声明，不能由任意上游字段自动成为表达式可访问属性。

### 4.2 兼容与错误边界

V1–V3 的已发布 schema、算子、内容摘要和回放解释保持不变。V4 复用词法作用域、集合算子
与工作量控制的实现，但不让旧规则自动获得新字段或改变比较结果。
完整报告及 decision envelope 沿用既有协议；新增事实证据通过兼容的元数据/产物引用投影。
独立工具事实协议有自己的版本，不能直接替换现有 diagnostics 输入格式。

新规则要求 V4 能力；旧 runtime 必须明确拒绝，不能退回字符串比较或跳过。
有界十进制的编码、规范化、单位和比较规则以 TFA-02 为准；这里不再定义第二套转换算法。
原始指标若经过上游舍入，协议保留其测量定义，不以精确序列化比较宣称恢复了上游数学精度。
固定版本 dryer 在输出 12 位小数之前筛选原始相似度，因此 V4 source 的范围验证还检查
舍入前的最小原始值：对消费下限向上取最近的 12 位十进制格点 `q`，要求采集下限为零，
或不大于 `q - 0.0000000000005`。该计算在有界整数中精确执行，并同时用于规划与消费阶段。
这是采集完整性的保守条件；实际记录与策略阈值仍按 TFA-02 精确比较。
边界反例及固定版本证据见[原生工具验证记录](../test/tool-facts-native-validation.md)。

报告解析的字节、深度、记录数与时间预算和表达式的 source、集合、节点及工作量预算分别计账。
复用集合不能绕过各消费者实际遍历的预算；截断报告不能充当完整集合参与断言。

## 5. 首批适配与独立验证

首期适配 crapper 与 dryer；工具、依赖、parser 摘要和实际正反例见
[原生工具验证记录](../test/tool-facts-native-validation.md)。原生执行支持 Linux x86_64，
固定 language-pack 1.25.2 和已验证的 Rust parser，使用私有缓存与无效本地 manifest 禁止下载。
其他平台仍可使用离线归一化和桥接程序；未覆盖的原生执行形态明确返回不支持。
上游项目仍为外部工具；本仓库只维护 Rust 解析、归一化、协议及独立验证逻辑。
工具安装和依赖准备是显式环境步骤，不由报告解析或 Gate 执行时隐式下载。

| 适配路径 | 对应需求 | 实现设计 |
| --- | --- | --- |
| crapper 函数指标 | TFA-02、TFA-03、TFA-06 | 先用已有执行边界产生并验证覆盖率，将已验证产物提供给上游的已有覆盖率模式，再归一化函数指标；字段缺失和未覆盖分别处理 |
| dryer 重复代码对 | TFA-05、TFA-06 | 保留双侧位置、候选范围、相似度及采集过滤条件；由策略断言阈值，解析警告或未完成范围保留为执行缺口 |
| mutator 结果 | TFA-07，后续 | 未分类 killed 只能作为有来源的建议性观察；增加独立逐变异执行证据前，不接入严格 test-effectiveness 阻断 |
| 模型检查结果 | TFA-07，后续 | 后续协议保留模型、配置、检查边界和反例产物，unknown 不变成布尔断言结果 |
| uml-viewer | TFA-07，后续 | 消费结构与指标证据；现有架构 gate 继续裁决依赖约束，首期不集成 GUI 或伴随 Agent |

对 Rust 小型隔离仓库建立干净版本、已知缺陷与未参与适配器调试的独立反例；预期结果独立编写。
除普通成功/违规，还验证缺覆盖率、解析警告、部分结果、阈值边界、源码/测试/依赖变化以及超预算。
工具观测、转换器自测和规则裁决分开记录，保留原始输出而不是仅保留最终分数。
首期仅以建议/影子方式评估检出、误报、incomplete 与延迟，阻断采用继续走既有策略生命周期。
本设计不为未经试点的工具写入生产阈值，也不把工具 README 的阈值直接当作仓库要求。

## 6. Bazel 构建与独立入口

对应 TFA-08，首期保留根 Cargo package 和 Cargo.lock，通过显式第二个 `[[bin]]`
提供 `qualitygate-tool-bridge`。根 package 设置 `default-run = "qualitygate"`，保留现有
`cargo run -- ...` 与 `//:qualitygate` 的使用方式。该名称已在开发构建中注册；现有二进制发行包不会自动包含它。

桥接程序提供有界“原始工具报告 → 工具事实协议”的转换入口，不隐式运行工具或安装依赖。
接口解析属于 interfaces；文件访问与工作协调通过既有能力边界；格式处理属于 adapters。
Qualitygate 本身可直接调用同一归一化库，不需要再启动一个桥接子进程。
独立入口可供其他调用方使用；离线输出始终为 `complete: false`，不能冒充第 3 节的真实运行绑定。

Bazel 为纯契约、归一化库、桥接 binary 和测试声明明确 targets、visibility 和实际外部依赖。
不把所有 CLI 依赖无差别传给每个新增 target；不同工具格式不各自复制调度或协议库。
上游 Python 工具作为固定版本的运行依赖另行准备，构建桥接 binary 不会证明这些工具已运行。

架构扫描已从原来的 CLI lib/main 扩展到 reviewed production target inventory，
并检查未登记入口和源码不可达的反例。业务模块归属、domain I/O 限制、
文档行数、Miri 纯模块清单及 Cargo/Bazel 测试覆盖均随入口清单检查。
CI 与仓库策略使用同一组全产品构建和测试目标，保持单元与集成测试独立。
同包新增 `[[bin]]` 会进入 Cargo 包的 target 元数据，必须验证 `cargo package` 与
`cargo install` 的实际行为；不能用 `default-run` 推断安装时只会选择原 CLI。
GitHub 二进制附件及 Skill 归档使用各自显式构建/打包清单，不会因 Cargo 新增 target
自动交付桥接程序。每种发行介质必须明确记录两个入口的包含或排除及其验证结果；
未发布的桥接入口只能描述为可从源码构建，不能宣称它已随现有 CLI 包提供。

首期“多个可独立构建目标”不宣称已经有多个独立 Cargo 产品包。
仅在后续明确需要独立版本、依赖集合和发布时，再设计 workspace 迁移：
保留一个锁文件、按 package 获取 Bazel 依赖、扩大所有质量门禁和源入口 inventory，
发布流程按包名定位版本。该迁移不属于本次 Spec 承诺的首期实现。

## 7. 实现验收与后续边界

需求文档的验收矩阵关联纯契约、适配器、执行集成、真实工具和构建入口测试。
CodeSpec 地图通过 `relay-knowledge map directory` 维护。文档检查仍包括相对链接、锚点、围栏、
需求编号引用、地图与索引一致性，以及 authored 文件不超过 1000 行。

首期实现分为“类型化事实与协议 → V4 消费 → crapper/dryer 接入及独立构建目标”。
每步须提交相应正反例和需求到测试证据，运行仓库既有质量门禁后才能登记工程完成；
现有策略的采用和阻断启用仍为另外的流程。Miri、ASan、完整 gate 结果与试点效果分别记录。
现行策略只扩大产品构建清单，不自动启用 crapper/dryer 阈值或变更策略采用状态。
首期原生适配面为已独立盘点的 Rust 函数；语言、实体排除及上游舍入限制随协议与文档保留。
验证成功不能替代真实生产试点或完整 Clean Code/DRY/SOLID 语义证明。
