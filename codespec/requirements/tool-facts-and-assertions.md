# 工具事实与可组合断言规格

> 文档版本：v0.1；日期：2026-11-17
> 评审基线：`900c795b893da5af472662bacef04cac82c0b270`

状态：**Draft / Not implemented**。本文件描述待实现能力；不表示已发布 V4、
已接入下列工具，或已完成任何新增验收测试。

本文是 TFA-01 至 TFA-08 的唯一行为规范。对应设计见
[工具事实与断言设计](../design/tool-facts-and-assertions.md)，
设计与示例必须引用这些条款，不得另行改变它们的语义。
本规格细化[产品需求](qualitygate-cli.md)，保留现有所有权、证据和质量要求。

## 背景与目标

现有命令执行、不可变快照、工具身份、产物摘要及外部报告归一化可继续复用。
现有 V3 支持有限集合、词法变量和组合表达式，但 named source 主要来自源码选择器，
公开字段为 name、text、path，不能直接对外部工具的数值和关系事实进行类型化断言。

目标是让项目规则声明“复杂度不超过阈值”“克隆关系不跨越指定边界”等要求，
工具负责产生事实，Rust adapter 负责验证并归一化，规则负责作出策略判定。
修改策略阈值应无需修改工具实现或重新编写报告解析器。

本规格同时约束同一 Cargo package 增加 Bazel 辅助 binary 的工程接入，
使多项目输入和新增入口不会绕过现有质量门禁。
它不定义任意代码插件、远程调度平台、通用图数据库或完整形式化证明系统。

## TFA-01：范围与扩展路线

1. 仓库拥有的实现保持 Rust；调用外部项目的既有工具属于命令执行边界。
   不在仓库中重写或复制 crapper、dryer、mutator 的分析算法。
2. 首期交付路线为“显式配置外部工具 → 有界执行 → Rust adapter →
   类型化事实集合 → 有限规则断言 → 既有 gate/report”。
   配置必须明确工具 argv、cwd、输入、输出和版本采集方式；不隐式安装工具。
3. crapper、dryer 是首期候选 adapter；能否进入严格门禁须满足 TFA-06。
   候选身份不是支持声明，存在工具输出不确定性时不得伪造完整结果。
4. mutator 首期不作为严格验收 producer；uml-viewer 后置为查看能力。
   首期不新增模型检查器 adapter，也不声称已有 invariant 证明能力。
   未来扩展可复用事实边界，但必须另行登记事实 schema、测量定义与验收场景。
5. 用户自定义规则仍是受限数据。禁止动态执行脚本、加载任意共享库、
   任意 JSONPath 字段访问，以及由规则表达式发起进程、文件或网络 I/O。

## TFA-02：类型化事实与来源身份

1. 内部事实流必须使用 Rust 类型契约，不能通过自由形式的
   `CheckResult.metadata` 键值重新解析来驱动后续规则。
   报告可以保留兼容投影，但报告投影不成为内部事实的第二份权威。
2. 每个事实集合必须绑定 producer check ID、事实 schema ID/version、
   snapshot identity、工具身份与版本、原始产物摘要和采集范围。
   复用已有身份采集与校验能力，不将工具自报的 snapshot 字符串视为绑定证明。
3. 集合内记录必须有稳定、无冲突的身份、所属 project/target 和可核实的位置。
   指标至少声明指标名、数值、单位及测量定义；关系记录至少声明两端身份与位置。
   不得把两个位置的克隆关系压成一个无法追溯另一端的普通 diagnostic。
4. schema 是版本化、封闭的事实契约，公开字段名及其类型由注册的 adapter 契约决定。
   未知 schema、未知字段、缺少 required 字段、重复身份和类型冲突必须拒绝。
   不允许通过一个无约束 `properties` 对象绕开契约。
5. 首期规范两种事实形状：数值 metric 与 clone pair。
   adapter 必须精确声明它实际提供的字段；不能从工具名称推断它必然提供某项指标。
   mutation、invariant 等未来形状不能冒用这两种 schema。
6. 已解析的记录和已验证的完整性必须一起交给消费者。
   单独保存一组数值、忽略其 producer 或范围绑定，不构成可接受的规则输入。
7. 数量采用 `i64`，计数必须非负。非整数采用有限精确十进制：有符号十进制系数最多 38 位，
   scale 为 0 至 18；值为 coefficient × 10^(-scale)。
   系数不能带指数或多余前导零；禁止负零；零规范化为 coefficient=0、scale=0；
   scale 大于零时去除系数尾随零并相应减小 scale。
8. adapter 从原始报告数值 token 无损规范化，不先转为二进制浮点再比较。
   可无损表示的指数形式可以规范化；超出上述范围、NaN、Infinity 或不可解析数值
   产生 incomplete，不饱和、不截断、不舍入成可通过阈值的数值。
9. 比较在声明单位内精确执行；整数可无损提升为十进制。
   比较中间量也必须有界，不能因整数溢出改变结果。
   不默认换算百分数、比例或其他单位，也不引入容差 epsilon。
10. 精确性只针对工具已序列化的观测值。工具内部已发生的舍入或截断不可恢复；
    adapter 必须记录工具版本与该测量定义的已知精度限制。
    规则只能声称其比较满足此测量定义，不能据此声称底层数学量具有更高精度。

## TFA-03：事实采集与策略判定分离

1. 事实 producer 的 pass 表示“声明范围内的事实采集与校验已完整完成”。
   consumer rule 的 pass 表示“对这些事实求值后的策略断言成立”。
   两者必须在报告及说明中可区分。
2. 采集到 complexity 超阈值或 clone pair 不得直接使专用事实 producer fail，
   从而阻止消费者解释这些事实。策略违反由对应规则产生诊断及 fail。
   现有 lint/report producer 的语义保持，不全局放宽 `depends_on` 的通过条件。
3. 进程终止状态符合 adapter 执行契约只是采集完成的必要证据之一。
   缺失报告、未完成范围、格式错误、工具错误及超时必须阻断，不能作为完整空集合。
   原生 findings exit code 只有经 adapter 验证属于已完成运行且产物一致时才能接纳；
   不能单独配置一个退出码来证明采集完成。
4. 生产者应提供声明范围内的原始观测和库存，而非只提供被工具阈值筛出的 findings。
   consumer 改变策略阈值时，既有完整事实应足以重算策略结果。
5. 若工具只能输出经过阈值或其他配置筛选的结果，adapter 必须明确记录这一限制，
   并拒绝需要更完整库存的断言；不能用“无 findings”推导任意阈值均成立。
   允许的工具采集选项必须与消费规则能力一致，并计入采集身份。
6. 事实生产者本身的输入或产物最终重验证失败时，消费它的结果也不得保留可信 pass。
   最终 gate 必须保持 incomplete；不能只在报告上附一条 warning。
7. 同一运行内，同一 producer 的同一采集范围只执行一次，其已验证事实可供多个
   显式依赖的规则共享；各规则独立求值、计量预算及裁决。
   首期不提供跨运行缓存，不用共享事实为来源过期或范围不符开例外。

## TFA-04：V4 有限表达式与规则扩展

1. 新能力使用独立的项目规则 schema V4 和 runtime capability
   `rules.operators.v4`。这些名称在本 Draft 中仅为拟议接口。
   不修改已发布 V1、V2、V3 的 schema、摘要、默认值或表达式含义。
2. V4 保留现有有限集合与词法变量模型；source 必须声明 producer check、
   事实 schema/version 和 project/target 范围。
   配置规划验证 producer 存在、位于该规则显式 `depends_on` 内，
   且 producer 的声明输出与 source 类型匹配。
3. 源码集合与工具事实集合可在同一表达式中组合，但字段访问受各自类型约束。
   `all`、`any`、`filter`、`map`、`count`、`distinct`、`subset` 和比较
   仅在各算子的既定类型规则内使用；不增加任意递归或用户函数。
4. source 的字段只返回声明的 Bool、整数、精确十进制或 String 类型。
   字符串不能隐式转数字；数值不能通过 text 字段或正则匹配替代数值比较。
   同一字段在不同 producer 间比较时，必须具有相容的 schema、单位与测量定义。
5. 数值及其比较严格遵守 TFA-02，不另设工具专有的隐式转换或容差。
6. V4 实现可复用现有引擎，但 V1–V3 必须继续使用其原有验证及求值语义。
   旧 runtime 对 V4 明确报告不支持，不能降级成 text rule 或空 pass。

## TFA-05：完整性、范围、空集合与预算

1. 采集范围必须来自已选 snapshot 和显式 project/target 库存。
   adapter 校验工具实际报告范围是否覆盖声明范围；producer 自报 `complete=true`
   不能替代这项校验。部分记录、部分文件或部分目标完成时不得整体标记 complete。
2. source 只能消费已声明依赖且完成采集校验的事实。
   producer 未运行、被跳过、失败、超时、snapshot 不匹配或来源不明均不可消费。
   不允许读取旧运行遗留文件作为本次运行的事实。
3. 必须区分“完整范围内确实没有记录”和“尚未获得记录”。
   `all([])=true`、`any([])=false`、`count([])=0` 只适用于经验证的完整集合。
   要求至少一个实体的规则必须显式断言 `count >= 1`；不改变逻辑算子的空集语义。
4. 路径筛选、变更范围和目标筛选必须保留证据范围。
   未采集的目标不能被过滤成空集后通过；也不能将局部采集的结论显示成全仓库结论。
   不支持某种范围映射时必须报告能力不足，不猜测目标到文件的归属。
5. 输入与原始报告沿用已有有限大小、进程输出、超时和路径约束。
   新的规范化记录须计入总预算，不能因分成多个文件或 producer 而绕开消费者预算。
6. V4 每条规则共用以下上限：16 个 named sources、50,000 条输入/中间记录、
   32 MiB 事实/物化数据预算、32 MiB 扫描字节、100,000 次运算、
   256 个表达式节点、16 层表达式深度；进程内 source 收集/归一化工作与表达式求值
   各受 30 秒预算约束。外部工具执行继续受既有 `timeout_seconds` 与总体执行预算约束，
   不因本条把覆盖率或其他外部命令的超时强制改为 30 秒。
   记录身份、字段名、字段值、位置、关系两端和保留的来源信息均须计入对应字节预算。
7. 超预算、超时或诊断截断不能变成成功的抽样结论。
   保留已有 bounded diagnostics 和 incomplete 行为，明确报告实际执行边界。
8. 快照、policy、工具身份和产物的校验继续由既有能力边界承担。
   新 adapter 不得自行绕过 InputGuard、受限路径或最终证据重验证。

## TFA-06：候选工具的接纳边界

1. crapper 的候选用途是提供工具实际公开的代码/测试风险相关数值观测；
   dryer 的候选用途是提供重复代码及 clone pair 观测。
   首期 adapter 必须钉住已验证版本、原始格式和测量定义。
   本规格不把工具 README 中的描述当作其所有版本都满足契约的证明。
2. 每个 adapter 接纳前必须验证正常输入、违反策略的完整事实、空结果、
   malformed/truncated 输出、工具错误、超时和不完整库存。
   必须证明工具默认筛选不会使消费者看不到所需记录；否则缩小可声明能力。
   crapper 的覆盖率输入必须有独立执行完成和产物校验证据；
   “有效报告明确记录零覆盖率”与“报告缺失、无法读取或不完整”必须分开。
   dryer 的采集相似度下限不得高于消费规则需要检查的下限：
   例如以 0.82 过滤采集时，不能证明“没有相似度至少为 0.80 的克隆”。
   `min-lines`、`min-nodes`、语言支持及排除的实体类型也必须进入采集范围声明；
   完整分析满足最小尺寸条件的函数，不能推出未采集的小函数或闭包没有重复。
3. adapter 保存原始产物摘要与记录位置，既能追溯工具结果，也能显示消费规则的断言。
   本规格不宣称 Clean Code、DRY、SOLID 等设计原则已被这些工具完整证明。
4. 首期只以建议/影子方式观察工具检出、误报、incomplete 与延迟。
   工程验收通过不自动将候选规则设为生产阻断项；采用仍须遵循现有策略生命周期。
   不将上游工具默认阈值直接设为本仓库的 required policy。

## TFA-07：证据局限、后续能力与验收状态

1. mutator 在首期明确为“不支持严格验收”，不能作为 pass 的依据。
   后续接纳需另行规定 mutant 总库存、计分分母、排除项及原生状态映射。
   killed、survived、no coverage、timeout、error 必须分开；
   timeout/error 不得计作 killed，也不能无证据移出分母来提高分数。
2. 若将来接入模型检查，unknown、资源耗尽、未穷尽范围不得映射为
   invariant=true 或普通 invariant=false。成功结论必须绑定模型、配置及搜索边界，
   反例必须引用有摘要的产物。这是未来接纳条件，不是首期支持声明。
3. uml-viewer 只列为后续查看候选。视图应消费已验证报告，不能改变 gate verdict，
   也不能以可视化是否生成成功替代分析与证据校验。
4. 本次只交付 Spec 和索引，不交付 Rust 实现、运行时 schema、配置迁移或新测试。
   下表所有场景均为 **planned**，不是已有测试或当前能力的证据。
5. 后续实现须保留 V1–V3 正反例、报告兼容性、source digest 和既有 CLI 行为。
   V4 的 schema 导出、能力发现、语义验证、求值与文档必须同步且可核对。
6. 验收应区分 violation、incomplete 和不支持；未知信息不得按通过计入。
   单元、集成、真实工具运行与生产试点证据分别记录，不能互相替代。
7. 下列场景通过前不得声明对应 TFA 条款已实现。
   不允许为获得通过而重生成独立 golden、放宽工具契约或降低所需质量门禁。
8. 任何性能、误报率、成本或质量收益必须来自明确版本、输入与方法的实测。
   本规格没有这些实测结果，不给出推定改善比例。

## TFA-08：Bazel 多项目与第二入口

1. Bazel 辅助入口采用同一 Cargo package 的第二个 binary，复用既有 reusable core；
   不为此拆分新 Cargo crate 或复制 runner、snapshot、config、gate 实现。
   拟议名称为 `qualitygate-tool-bridge`，只提供原始报告的有界归一化；
   不运行上游工具、不安装依赖。Qualitygate 可直接复用同一归一化库。
2. 增加第二 binary 时显式设置 `default-run = "qualitygate"`，
   保持无 `--bin` 的既有开发入口行为。原 qualitygate CLI 的参数及报告协议不变。
3. 各入口只承担 interfaces 能力，业务能力仍按既有 owner map 分配。
   Bazel ownership entry points 继续位于 `bazel/owners`；
   不因新入口增加 role wrapper 或产生 adapters → interfaces 依赖。
4. 多项目事实身份必须包含明确的 project、Bazel repository/package/target，
   涉及配置差异时同时包含 configuration 身份。
   同一文件被多个 target/configuration 使用不能直接按路径合并。
   非 Bazel 项目使用明确 project 身份，不伪造 Bazel label。
5. 第一版只要求已声明项目/目标的采集与事实归属；
   不隐含承诺全工作区自动发现、remote execution、通用依赖图或增量缓存。
   目标库存与事实范围仍必须满足 TFA-05。
6. Cargo、Bazel、CI、覆盖率和发行/安装清单必须显式处理两个 binary 及共享核心。
   必须验证新增 binary 对 `cargo package` 与 `cargo install` 的实际影响。
   每种 GitHub/Skill 等发行介质分别注明包含或未包含桥接入口，不强制修改所有既有发布包；
   未发布的入口不能被描述为已可下载或已安装。
   不得以只测试默认 binary、只构建既有 target 来宣称新增入口已经交付。
   第三方外部工具的安装与许可保持独立，不因打包 adapter 而声称已随包交付。
7. 实现时必须运行既有 fmt/check/clippy/test 全目标全 feature 门禁及 90% Rust
   行覆盖率门禁，并验证新增入口对应的 Bazel 构建、测试与架构 owner 合同。
   Miri 继续验证纯 domain，ASan 继续验证 native unit tests，保持各自独立。
   不能通过减少 required target、测试选择或覆盖率分母来通过。

## 计划验收矩阵

| 场景 ID | 条款 | 计划验收场景及结果 | 状态 |
| --- | --- | --- | --- |
| TFA-A01 | TFA-01、02 | 外部工具经受限 runner 产生原始报告，Rust adapter 输出带来源身份的 typed facts；未配置工具不隐式执行。 | planned |
| TFA-A02 | TFA-02、04 | 未知 schema/字段、重复记录、缺 required 字段及跨 source 类型错配被拒绝。 | planned |
| TFA-A03 | TFA-02 | 整数与十进制阈值恰好相等、最小可表达差异、不同 scale 的等值数均精确比较；无 epsilon。 | planned |
| TFA-A04 | TFA-02 | 超精度、超 scale、NaN/Infinity、溢出及单位不相容不产生 pass；上游舍入限制保留在测量定义中。 | planned |
| TFA-A05 | TFA-04、07 | V1–V3 原 fixtures 与 schema digest 保持；旧 runtime 拒绝 V4；新 runtime 能明确发现拟议 V4 能力。 | planned |
| TFA-A06 | TFA-03 | 完整超阈值 metric 的 producer pass、consumer fail；调整规则阈值后无需改 adapter。 | planned |
| TFA-A07 | TFA-03、06 | 仅输出阈值筛选 findings 的工具不能满足完整 metric source；不能以空 findings 推导通过。 | planned |
| TFA-A08 | TFA-03、05 | 未被契约接纳的非零退出、timeout、malformed/truncated/缺失报告保持 incomplete；经验证的 findings exit code 可完成采集。 | planned |
| TFA-A09 | TFA-05 | 完整空集合遵循 all/any/count 定义；缺目标与未运行不能伪装为空集合；显式最低数量断言失败。 | planned |
| TFA-A10 | TFA-02、05 | 未声明依赖、被跳过 producer、错 snapshot、旧产物、最终输入变更不能支持 consumer pass。 | planned |
| TFA-A11 | TFA-05 | 多 producer/source 共同计费；记录数、字节、时间、深度和运算达到边界时行为一致，超限 incomplete。 | planned |
| TFA-A12 | TFA-06 | crapper/dryer 已钉住版本的独立正反例验证字段、库存与位置；缺失能力明确拒绝。 | planned |
| TFA-A13 | TFA-02、06 | clone pair 保留两端位置，项目范围筛选及 count/compare 不丢失另一端证据。 | planned |
| TFA-A14 | TFA-07 | mutator 不能用作首期严格验收；unknown/timeout/error 不被呈现为 mutation 或 invariant 成功。 | planned |
| TFA-A15 | TFA-05、08 | 同一文件属于不同 Bazel target/configuration 时事实身份独立；缺一个目标库存则 incomplete。 | planned |
| TFA-A16 | TFA-08 | 同包第二 binary、default-run、Cargo/Bazel/CI/coverage/发行清单覆盖及架构 owner 合同均被检查。 | planned |
| TFA-A17 | TFA-03、05 | 两条规则消费同一 producer/范围，工具只执行一次；规则分别裁决并独立计费；范围或 snapshot 不符不复用。 | planned |
| TFA-A18 | TFA-03、06 | crapper 使用有效的零覆盖率报告保留 coverage=0；缺失或不完整覆盖率报告产生 incomplete，不能伪造零。 | planned |
| TFA-A19 | TFA-03、06 | dryer 以 0.82 采集过滤而消费规则要求 0.80 时被拒绝；完整采集后 0.81 的 pair 应使该规则失败。 | planned |
| TFA-A20 | TFA-05、06 | dryer 的最小行数、节点数和实体排除随范围保留；要求未采集的小函数或闭包时报告能力不足，不以空结果通过。 | planned |

## 来源与解释边界

设计动机参考用户提供的《Code Quality Engineering in the Age of AI Coding》
（2026-11，25 页）：第 16–20 页的独立工具验证与采用边界，
第 21–23 页的工具、规则与断言证据衔接；并结合当前仓库的有限 DSL、快照证据与所有权合同。
本轮仅完成源码与文档评审，未运行上游工具，也没有效能试点结果。
以下外部来源用于确认候选工具及工程接口；它们不是本项目已通过验收的证据。
实际 adapter 接纳时必须钉住工具版本或提交身份，不能把移动分支链接当作不变协议。

- crapper：[官方 README](https://github.com/unclebob/crapper/blob/master/README.md)、
  [分析实现](https://github.com/unclebob/crapper/blob/master/src/crapper/analyze.py#L23)、
  [CLI 实现](https://github.com/unclebob/crapper/blob/master/src/crapper/cli.py#L289)。
- dryer：[官方 README](https://github.com/unclebob/dryer/blob/master/README.md)、
  [CLI 实现](https://github.com/unclebob/dryer/blob/master/src/dryer/cli.py#L306)、
  [扫描实现](https://github.com/unclebob/dryer/blob/master/src/dryer/scan.py#L74)。
- mutator：[worker 实现](https://github.com/unclebob/mutator/blob/main/src/mutator/workers.py#L322)、
  [计分实现](https://github.com/unclebob/mutator/blob/main/src/mutator/metrics.py#L67)、
  [引擎实现](https://github.com/unclebob/mutator/blob/main/src/mutator/engine.py#L77)。
- 查看工具：[uml-viewer 官方仓库](https://github.com/unclebob/uml-viewer)。
- Bazel Rust 接入：[rules_rust crate_universe 与 Bzlmod](https://bazelbuild.github.io/rules_rust/crate_universe_bzlmod.html)。
