# 项目与工具报告

[English](../../en/02-reference/03-projects-and-tool-reports.md) · [本卷目录](README.md)

项目适配器提供不能从单个源码文件安全推断的事实。Maven 分析解析模块、声明依赖、producer 关系和
编译使用证据；Python 分析比较已安装 distribution 与声明 requirement，并保留解释器和环境身份。
清单缺失、模块归属不明确、工件无法解析或 requirement 语法不支持时保持未完成。

已安装的 Python `METADATA` 必须是最多 2 MiB 的普通文件。采集前先检查类型和大小，打开后
复核句柄，并通过有界读取检测文件增长。已有 FIFO、socket、目录或超大文件会使 producer
保持未完成，即使安装命令成功退出；已采集的命令日志与安装报告保留，下游检查阻断。
该预检不提供操作系统文件读取的硬性截止时间，也不保证抵御所有并发路径替换。
真实 pip 验收与合成异常产物回归分别验证。

## 统一报告契约

外部检查声明固定命令、工作目录、超时、认可状态、工具/版本证据、报告路径、解析 profile 和模式。
报告只能从快照物化执行中读取，并受文件大小、数量、诊断数和解析时间限制。缺失、过期、格式错误、
截断或快照不匹配的报告不能静默通过。

所有 XML 适配器入口共享最多 32 层元素深度（根元素计入）和 100,000 节点上限（文档根节点计入）。
范围包括 JUnit、Checkstyle、PMD、SpotBugs、Cobertura、JaCoCo、配对 JUnit、Maven 快照清单、
effective model、依赖使用分析，以及 japicmp 比较和 API 清单。递归树解析前先迭代检查 token 深度；
注释、CDATA、处理指令与转义属性值不增加深度。各调用方保留原有字节预算：生成的报告/模型最多
2 MiB，快照清单使用选定的快照获取预算。超深或节点过多返回未完成证据（退出码 2），保留已采集
原始报告并阻断依赖检查；各报告格式的 DOCTYPE 处理仍归对应 profile 所有。

归一化结果保留工具与规则 ID、严重级别、消息、URI、位置、指纹、计数、原报告摘要、命令证据和
完整性。退出码与报告内容一起判定；认可的 producer 退出码不能覆盖 fatal 或 incomplete 报告。

声明的报告与项目输出文件不能覆盖已选快照输入。规范化路径冲突（包括 `./` 别名和基线输出）在
报告准备修改文件前拒绝，`doctor` 分别报告当前或基线冲突。stdout 报告路径仅是证据标签，不占用
可写文件。工具探测与报告准备完成后、生产命令启动前验证输入完整性。前序命令生成的输出仍会被
清除，每个生产者都必须重新生成报告。生成报告应放在已选输入之外，例如 Git 忽略的构建目录。

每项检查的报告路径（含 stdout 标签）与项目输出路径按仓库相对路径规范化后必须唯一；例如
`target/report.json` 与 `./target//report.json` 表示同一输出。重复声明在配置加载时拒绝，
不会启动探测或生产命令。需要执行基线时，其文件输出（含回退到当前路径的输出）独立检查唯一性；
当前与基线在各自工作区中可以使用相同路径。单个合法别名继续接受，声明路径不被改写。

测试计数与选中的覆盖行数会累加到检查的 `matched_entities`。每次累加都必须能够用运行时的
无符号计数表示；溢出使检查保持未完成，不发生计数回绕或 panic。先前的违规与原始报告工件继续
保留，依赖检查保持阻塞，直到生产者提供可表示的证据。debug 与 release 构建采用相同判定。

JUnit 会逐层核对每个套件声明的 `tests`、`failures`、`errors` 和 `skipped` 与用例明细，
包括嵌套套件；这些计数属性仍可省略。用例必须直接属于 testsuite，结果类型必须唯一；同一
用例的多个 failure 记录只计为一个失败用例。计数或结果矛盾、重复 error/skipped、游离结果
记录均使验证未完成（退出码 2），保留原始报告并阻塞依赖检查。明细一致的失败用例仍属于
违规（全部必需检查完成时退出码 1；必需依赖被阻塞时仍为 2）。普通 JUnit 仍允许缺少
file、classname、name 和 failure type 元数据；
配对测试有效性证据继续保留额外的用例身份与源码要求。

## 覆盖率与静态分析

覆盖率判定使用精确的已覆盖数与总数，按配置阈值的规范化十进制值比较；展示百分比的舍入不参与判定。
100% 要求每个测量项都被覆盖，99.9% 接受恰好 999/1000；大计数不能通过舍入抹去未覆盖项。
即使展示百分比舍入后恰好等于阈值，诊断仍包含精确计数。空测量集合继续保留 null 百分比。

覆盖率在声明 profile 内支持原生 coverage.py JSON、Cobertura XML、JaCoCo XML 与 LCOV。阈值基于
covered/total 行事实，并保留零计数。空报告、内部计数矛盾、源码映射缺失、重复冲突条目或解析超限
均为未完成。高覆盖率不能证明行为正确。

LCOV 汇总计数不一致时，错误会指出 `SF` 源文件、计数器（`LF`、`LH`、`BRF` 或 `BRH`）、
声明值及从明细计算出的值，并保留原始报告，检查仍为未完成。真实生产者也可能输出这种矛盾：
当前锁定的 Rust/LLVM 导出器在闭包与外层函数重叠时，可以为 4 条 `DA` 记录输出 `LF:5`。
应检查错误指出的源文件段并重新生成一致的生产者证据；调整覆盖率阈值不能消除报告矛盾。

SARIF 保留 run、工具版本、规则、result kind、消息、位置与 partial fingerprint。ratchet 会在两个
不可变快照上重新运行相同分析器，再按配置比较 `(tool, rule)` 计数或稳定身份。抑制、缺失和仅基线
存在的结果保持可区分。

基线执行、输入验证或报告解析失败时，仍按原有字节限制采集当前文件与 stdout 报告并保留原始工件。
不会把失败基线当作空基线进行比较：检查仍为未完成，依赖检查保持阻塞。若当前报告采集也失败，
会同时保留采集错误与基线失败原因；错误发生前已成功采集的报告仍可查看。
这一行为同时适用于 `ratchet` 与 `new_diagnostics` 检查。

参考棘轮覆盖 Clippy、ESLint、golangci-lint、Ruff、GCC/Clang analyzer SARIF、Checkstyle、PMD 与
SpotBugs。各 producer 的完成标志、文件清单、错误记录、路径映射和版本一致性属于完整性契约。

## 类型化工具观测

命令可声明单个 `tool_facts` 输出，与诊断报告和项目报告互斥。闭合契约包含 `adapter`、
`schema`、`path`、`from_stdout`、`tool_id`、`tool_version`、`scope` 以及精确的源码 `paths`。
工具事实协议 v1 将库存范围固定为有函数体的 Rust `function_item`，通过 `crate_root`、
`crate_name` 独立盘点。闭包、block 内局部函数以及名为 `tests` 的模块内函数不属于该范围。
身份歧义与不支持的泛型/限定路径 impl self 类型会拒绝采集，不是静默排除；非 Rust 输入不支持。
项目、可选 target 和 configuration 均显式声明，不扫描 Bazel 缓存或自动发现工作区。

`adapter: tool_facts` 接受 `schema tool-facts-v1` 导出的 JSON 协议。指标生产者声明完整的指标、
单位与测量定义清单，库存中每个函数必须提供每个指标。克隆关系声明测量定义与采集筛选条件。
报告须携带与独立盘点一致的库存。可选 `context_path` 在执行前接收包含 `producer`、
`snapshot_digest` 和 `spec` 的有界 JSON 输入；通过配置的 argv 让工具读取它。
该文件不能覆盖快照输入或输出，执行时修改它会使生产者未完成。工具自行填写的身份不能替代
实际命令、版本探测、快照与留存产物摘要的绑定。

原生适配版本固定为 crapper `c757ec79a6c15dc815238bbd9c2f4601eb54d3cc` 和
dryer `66ff6d21a42c04afcad89c78a80066176d1294b0`。安装由调用者显式完成；应声明上游源码、
解释器和依赖锁的版本探测与输入。源码 commit 不等于 Python 与解析依赖版本已固定。
原生命令要求仓库根目录 cwd，并逐一传入声明的源码文件；不支持的参数在执行前拒绝。

当前原生执行验证范围为 Linux x86_64 与 `tree-sitter-language-pack` 1.25.2。
`tools` 中声明 ID 为 `tree-sitter-language-pack` 的探测，使用与分析器相同的解释器，输出必须
恰为 `1.25.2`。`native_runtime` 声明 `pack_version`、选定快照中的 `parser_path` 与
`parser_digest`，仅接受官方 Rust 解析器摘要
`sha256:43b4551eedd58c54397f9571cacdefdac2ec077eaceb00a43977563c605d03f6`。
将该文件作为显式验证输入保留。门禁复制解析器到私有缓存，仅为子进程覆盖缓存、动态库和
manifest 三个路径，并提供无效的本地 manifest。解析器缺失时本地失败，不隐式下载。
缓存和留存证据均会复核；其他原生执行平台明确不支持。离线归一化与桥接程序仍可跨平台使用，
不要求配置原生运行环境。

crapper 必须使用 `--use-existing-coverage`，显式依赖已独立执行并成功验证完整 LCOV 的
`coverage_producer`。`target/coverage` 与 `coverage` 下只能发现已验证的 LCOV；会优先覆盖
LCOV 观测的 HTML 报告被拒绝。执行前后核对输入摘要，实际零覆盖率保留为 0，缺失函数观测
保持未完成。coverage 与 CRAP 的测量定义保留上游四位小数舍入限制。显式阈值对应的已声明
findings 退出码可以完成采集，超阈值由后续规则裁决。

dryer 显式提供与采集契约一致的 `--threshold`、`--min-lines`、`--min-nodes`；stdout 另需
`--edn`。事实保留双侧位置、节点数及上游十二位小数的分数定义。要求 0.80 相似度下限的
source 不能消费以 0.82 采集的事实，要求更小函数的 source 也不能消费已排除它们的结果。
dryer 先按未舍入分数筛选，再序列化为十二位小数。该固定测量定义下，消费下限 `t` 除须满足
通常的范围包含条件，还要求采集下限为零，或不高于
`ceil(t * 10^12) / 10^12 - 0.5 * 10^-12`。这是精确的舍入原像边界检查，防止遗漏向上舍入后
恰好达到消费边界的 pair；不改变数值比较，也不引入 epsilon。消费下限为 `0.82` 时，可声明并
实际按 `0.819999999999` 采集，或按零完整采集；按 `0.82` 采集不能支持相同的序列化消费下限。
通用 JSON 信封若声称相同测量定义，也须满足此条件。[原生验证记录](../../../codespec/test/tool-facts-native-validation.md)
保留了实际反例和独立的仅生产者 smoke 结果。
支持的实体范围不包括闭包与 tests 模块函数，不能据此声称这些实体中不存在问题。

完成采集后才向内部发布类型化事实；指标较高不直接让生产者失败。V4 规则声明 producer 依赖并
分别判断阈值，同次运行复用一次采集。最终输入、工具或留存工件失效时，在汇总门禁前阻塞依赖
断言。普通 lint 报告语义不变。工具策略先作为 warning/shadow 候选，经现有采用流程后才阻断
交付。严格 mutator 验收、模型检查和 GUI 不属于首期。

`qualitygate-tool-bridge --format dryer --input captured.edn --context context.json`
调用同一归一化库，不执行工具。context 使用 `NormalizeContext` 契约；离线转换无法验证执行，
因此输出始终为 `complete: false`。两个程序的构建、安装与发行包排除项见
[开发与质量门禁](../04-contributor-guide/01-development-and-quality-gates.md)。

## 兼容性、Bazel 与增量语义

Java 兼容性检查构建成对快照，并调用配置的二进制/源码兼容工具。任一侧构建失败、API 面无法解析或
工具未执行都属于未完成。

Bazel 是使用 Bzlmod 且与 Cargo 依赖对齐的额外可复现构建面，不替代 Cargo 必需门禁。缓存状态、
lockfile 行为、平台和选中 target 都属于证据。

覆盖率元数据 `<report>:mode` 记录实际测量模式：delivery 即使配置为 `full`，也记录
`changed_lines`；`<report>:configured_mode` 保留配置值。可复用核心的 repository 评估使用配置的模式，
计数和阈值判定均遵循实际测量模式。

`new_diagnostics` 先用当前保留的未改动诊断消耗基线同标识计数，再筛选交付诊断。
保留历史诊断并在变更行新增相同诊断时，新增实例仍会被报告；单纯移动已有诊断不会被当成新增。

增量模式必须声明含义：`new_diagnostics`、`changed_lines`、`affected_scope` 或 `full`。文件发生变更
不代表其历史诊断全部变成新增问题。缺少可比基线时，必需 ratchet 保持未完成，除非受信策略事先
允许且明确披露 full 回退。

---

[上一章：内置与自定义规则](02-rules.md) · [全书目录](../README.md) · [下一章：策略、任务与签名证据](04-policy-task-and-signed-evidence.md)
