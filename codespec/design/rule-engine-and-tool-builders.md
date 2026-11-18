# 规则引擎与 Agent 自建检查工具的技术评审

评审日期为 2026-11-18，代码基线为 `4d8537e24138c1fc4b2dcbfd3d21682fc0191fad`。行为约定沿用 [TFA-01 至 TFA-08](../requirements/tool-facts-and-assertions.md)。

**可以在被检项目的 `tools/` 中编写 Rust 检查器。** Agent 负责把检查方法写成可测试的程序，项目策略用 `checks[].argv` 显式调用。工具能直接报告违规，也能把已支持类型的事实交给多条规则独立裁决。当前扩展性最好的是增加同类检查、替换符合相同契约的生产者、调整规则阈值。新增实体类型或测量语义要先核对引擎能否验证其范围与完整性。

## 当前实际执行什么

本次用开发 CLI 1.3.0、仓库随附规则资产和本地策略运行 `capabilities --format json`、`rules list --format json` 和 `schema tool-facts-v1 --format json`。目录中有 138 条内置规则和 1 条项目规则，启用的规则为 `line-ending`。full profile 另选中 10 项命令，涵盖格式、编译、Clippy、单元与集成测试、selfcheck、仓库质量检查以及三个 Bazel 检查。运行时声明支持 V4 和工具事实 v1，仓库策略当前配置的事实生产者数量为零。[仓库策略](../../qualitygate.yaml) · [运行能力声明](../../src/qualitygate/check_planning/runtime_contracts.rs)

**部署时要核对实际二进制的能力。** 评审机已安装的 Linux x86_64 Skill CLI 同样显示 1.3.0，SHA-256 为 `c8e402349a10acf4eb0610912c921d51890e9dcbb6ecda88f161188888913da0`。它列出 V2、V3，查询 `schema tool-facts-v1` 返回未知子命令（退出 2）。用它执行下文项目自编的 diagnostics 检查器，仍能正确报告第二行违规（退出 1）。这两个构建都能接入普通命令，只有本次核实的仓库开发构建能执行下文 V4 事实链。[运行时兼容约定](../../skills/qualitygate-cli/references/runtime-compatibility.md)

仓库根目录目前没有 `tools/`。同名配置字段 `checks[].tools` 是版本探针列表，每个探针记录实际执行的 argv、可执行文件摘要、版本输出和声明的快照输入摘要。`doctor` 能预检已声明的工具与输入，`selfcheck` 运行内置夹具。`rules generate` 写入候选规则，`agent-run` 可以调用选定的外部 Agent。检查器源码由项目中的 Agent 编写，源码所在目录没有自动发现或自动启用语义。[探针实现](../../src/qualitygate/gate_execution/tool_evidence.rs) · [候选规则生成](../../src/qualitygate/rule_catalog/rule_authoring.rs) · [Agent 入口](../../src/qualitygate/agent_runtime/agent_run.rs) · [selfcheck 范围](../../skills/qualitygate-cli/references/selfcheck.md)

## 可扩展性取决于缺少哪一种证据

| 项目需求 | 最小接入面 | 增加工具或规则时的影响 |
| --- | --- | --- |
| 已有内置规则能表达，只有参数或适用范围变化 | 在策略中选择已注册的实现及其参数 | 目录中的多个规则可以复用同一 Rust 实现。新增内置算法涉及参数定义、执行分派和独立夹具。[参数校验](../../src/qualitygate/rule_catalog/builtin_validation.rs) · [执行分派](../../src/qualitygate/rule_evaluation/builtin_rules.rs) |
| 所需事实已经存在，只缺组合规则 | 在项目规则目录编写有来源绑定的版本化 YAML。V1 用已有选择与检查，V2 用有界表达式，V3 增加命名源码集合和词法变量，V4 可引用工具事实 | 改阈值、计数或同类型集合关系可复用原生产者。Schema、类型检查和求值器共同限制可用字段与算子。新增算子应同时覆盖校验、运行时、能力声明与旧版本回归。[版本化 Schema](../../src/qualitygate/rule_catalog/rule_schema.rs) · [项目规则求值](../../src/qualitygate/rule_evaluation/custom_rules/mod.rs) |
| 需要项目专属算法，只关心这一项检查的结果 | 用命令调用既有系统工具或项目自编程序。按用途选择 diagnostics、SARIF、JUnit、覆盖率等已有报告格式 | 项目实现检测逻辑，已有 adapter 归一化并产生该 check 的结果。直接按退出码验收也可用，但裸命令的非预期退出会产生违规诊断，无法仅凭一个非零值区分工具故障与业务违规。[报告格式](../../src/qualitygate/check_planning/model.rs) · [命令结果判定](../../src/qualitygate/gate_execution/commands.rs) |
| 多条规则需要对相同观测值施加不同策略 | 在策略 `checks` 中声明 `tool_facts`，配置 ID 与 `tool_id` 相同的版本探针。该命令的 `reports` 和 `projects` 须为空。规则以 `depends_on` 和 `fact_sources` 引用它 | 可自编程序输出 `adapter: tool_facts` 的 JSON。规划时核对生产者、schema、project/target/configuration、单位和测量定义。执行时可用 `context_path` 取得当次身份。一次成功采集供本次运行多个消费者使用。仅在任务中声明的生产者不能满足项目规则的绑定。[生产者校验](../../src/qualitygate/check_planning/tool_facts.rs) · [绑定校验](../../src/qualitygate/rule_catalog/tool_source_validation.rs) · [V4 接口与示例](../../skills/qualitygate-cli/references/operators-v4.md) |
| 需要新的原始格式、实体或关系供规则组合 | 原始格式能在项目工具中无损转换且满足现有库存时，可沿用协议。新实体或事实形状需要版本化的 Rust 契约、独立库存核验、适配器、字段投影及对应测试 | 当前事实 schema 只有 `function_metric`、`clone_pair`。通用 JSON 路径也按显式 Rust 源文件独立枚举有函数体的函数项，排除闭包、直接声明在块内的函数和内联 `mod tests { … }` 中的函数。外置 `src/tests.rs` 的文件名本身不触发该排除。遇到不支持的 impl 形态报不完整。其他语言仍可走普通报告。[事实契约](../../src/qualitygate/tool_evidence/contracts.rs) · [库存实现](../../src/qualitygate/tool_evidence/inventory.rs) · [V4 字段类型](../../src/qualitygate/rule_catalog/operator_v4_types.rs) |

原生 crapper、dryer 复用同一条采集链，开发版原生执行验证限定在钉住的工具、解析器和 Linux x86_64。独立程序 `qualitygate-tool-bridge` 能离线归一化已捕获的报告，输出保留 `complete: false`。需要绑定当前运行的事实时，入口是受计划控制的 producer。项目规则表达式只读取有界数据。[固定版本实测](../test/tool-facts-native-validation.md) · [桥接程序回归](../../tests/tool_bridge.rs)

如果检查器只服务于某个仓库，代码、配置和样例可随该仓库维护。如果检查器要随 Qualitygate 产品发行，就要按本项目的 Rust 能力归属、Cargo/Bazel 入口清单和相应测试接入。`qualitygate-tool-bridge` 是后一种选择的现成实例。[架构归属](../../AGENTS.md) · [双入口设计](tool-facts-and-assertions.md)

## 可靠性从计划延伸到最终裁决

| 执行环节 | 引擎提供的保障 | 对自建工具的意义 |
| --- | --- | --- |
| 选定策略与依赖 | 在选定快照/策略上验证配置，检查重复 ID、缺失依赖、重复边及环，按依赖闭包生成稳定顺序。来源段落有内容摘要，项目规则的采用另有绑定 | 在候选工作树里可验证工具输出是否满足规则的输入要求。纳入既有正式策略时，应按该策略保护检查器、manifest、锁文件及独立预期。版本探针列出输入可以追溯字节，采纳仍取决于策略来源。[依赖图](../../src/qualitygate/check_planning/dependencies.rs) · [策略装载与验证资产](../../src/qualitygate/policy_lifecycle/policy_loading.rs) |
| 实际执行与资源 | 在物化工作区用固定 argv 执行，限制进程输出和超时，取消时清理受控进程组或 Job。清理旧报告，核对输入身份，首次输入校验失败会持续使该物化工作区失效 | 将输出指定到生成目录或 stdout。探针、分析或解析失败应有可辨别的状态。进程控制与快照校验提供执行证据，宿主操作系统的隔离能力仍取决于运行环境。[runner](../../src/qualitygate/runner/mod.rs) · [InputGuard](../../src/qualitygate/snapshot/input_guard.rs) · [产物处理](../../docs/en/02-reference/03-projects-and-tool-reports.md) |
| 传递与复核 | 只有完成且通过的前置节点可供依赖继续。事实通过类型化运行结果传递，完成时复核快照、相关工具身份和保留产物，沿显式依赖传播失效并清除 verdict | 同一次运行中共用的生产者只执行一次。普通命令按计划串行，共享工作区，每个命令还有输入复核开销。工具越多，执行与读取成本会累加。下一次运行会重新采集。[工作流](../../src/qualitygate/gate_execution/workflow.rs) · [最终失效传播](../../src/qualitygate/gate_execution/fact_revalidation.rs) · [产物变更回归](../../src/qualitygate/gate_execution/fact_execution_tests.rs) |
| 裁决 | 对照独立 required 清单。有理由且状态一致的不适用项可跳过。缺必需结果、状态矛盾或全部跳过时 Gate 为 incomplete（2），同时保留已知违规。证据完整时，error 级违规为 fail（1），warning 可随 pass（0）保留 | 消费报告时同时读取 `complete`、`decision`、check 的 execution/verdict 及范围。对 delivery，full 表示选定的完整计划，定位型诊断仍受交付变更范围限制。[Gate 判定与回归](../../src/qualitygate/gate_execution/gate_tests.rs) · [范围反例](../requirements/skill-rule-boundaries.md) |

在一条 V4 规则内，各来源共用记录、字节及运算预算。不同消费者分别计费。上限包括 16 个命名源、50,000 条输入/中间记录和 100,000 步。事实生产者自身也有 50,000 条记录上限。例如完整枚举所有符合长度条件的函数对时，316 个函数产生 49,770 对，317 个产生 50,086 对，单个生产者已经无法完整提交后一库存。字节或时间预算还可能先耗尽。缩小采集范围会改变结论覆盖的范围。这些可计算的边界有助于选工具，端到端延迟仍需按实际项目测量。[记录与完整性校验](../../src/qualitygate/tool_evidence/normalization.rs) · [表达式预算回归](../../src/qualitygate/rule_evaluation/operators_v4_regression_tests.rs)

## 准确性取决于识别范围和测量定义

| 要判断的问题 | 实现或反例 | 结论的适用范围 |
| --- | --- | --- |
| 是否选中了符合业务含义的实体 | 隔离 Java 仓库只启用 `junit-naming`，方法名同为 `wrong`。直接 JUnit `@Test` 命中 1 个实体，Gate 退出 1。改为由 `@Test` 修饰的组合注解 `@FastCase` 命中 0 个，Gate 退出 0。无关的本地同名 `@Test` 命中 1 个，Gate 退出 1。三次均 complete | 语法提取使用已知注解名，可能漏选框架组合注解或误选同名注解。三例检验提取器的边界。没有执行 JUnit，也没有测量生产误报率。[测试实体提取](../../src/qualitygate/source_analysis/syntax/tests_in_source.rs) |
| 空集、多算或少算如何影响判断 | `all([])` 为真，`any([])` 为假，缺生产者或错快照则中止。对于指标，逐一核对函数位置 × 已声明指标并拒绝重复。对于 clone pair，在节点下限和相似度下限均为零时，可按长度入选的 n 个函数核对 n(n−1)/2 对 | 要求至少一个实体时显式断言 `count >= 1`。启用采集过滤后，位置合法与记录唯一无法独立揭示漏报的合格 pair。通用生产者算出的指标数值同样需要独立预期核对。[记录校验](../../src/qualitygate/tool_evidence/normalization.rs) · [空集与缺失反例](../../src/qualitygate/rule_evaluation/operators_v4_regression_tests.rs) |
| 阈值边界能否重算 | 数值按单位和测量定义比较，使用有界精确十进制。固定版本 dryer 先筛原始相似度再舍入，小夹具的原值 0.5263157894736842 被输出为 0.526315789474，若按后一个数采集就会漏掉该对 | 比较的精度限于已序列化观测。规划和消费均核对采集窗口能覆盖舍入边界。相同展示阈值不一定允许复用同一份采集结果。[原生反例与原始摘要](../test/tool-facts-native-validation.md#rounded-acquisition-boundary-counterexample) · [范围比较](../../src/qualitygate/tool_evidence/contracts.rs) |
| 增量通过能说明多少 | `ratchet` 重跑基线和当前分析器，诊断按工具与规则分组计数。同一组旧问题消失、新位置出现，总数可持平。跨规则的增减分别判断 | 计数未增长只能支持该分组的计数结论。若业务关心新增的具体位置或记录，应核对相应身份与差异模式。已列条款的 constraint coverage 同样只覆盖已绑定的义务。[比较逻辑](../../src/qualitygate/report_evidence/report_gate.rs) · [基线回归](../../tests/ratchet.rs) · [约束覆盖](../../src/qualitygate/constraint_coverage/coverage_evaluation.rs) |

### 已复现的契约差异

用只有 `pub fn alpha() {}` 的仓库和通用 `clone_pair` 生产者，声明节点下限、相似度下限均为零，输出完整单函数库存及零对。V4 消费者显式依赖它，要求至少 1 行的函数范围内无克隆对。只把 producer 的 `minimum_lines` 及信封同名字段从 1 改为 0，结果如下。

| 生产者声明 | `config --show` | `scan` 及保留的事实信封 | 导出的 `schema tool-facts-v1` 校验 | V4 消费者与最终 Gate |
| --- | --- | --- | --- | --- |
| 1 | 0 | completed/pass，`complete: true` | 通过 | pass，退出 0 |
| 0 | 0 | completed/pass，`complete: true` | `$.minimum_lines` 低于 1 | blocked，无 verdict，`Invalid acquired clone universe`，退出 2 |

第三组只保留同一零值生产者，去掉消费规则。此时 Gate 完整通过并退出 0，保留的信封仍含 `complete: true` 和 `minimum_lines: 0`。生产者配置的正数检查目前仅用于原生 dryer，通用 JSON 导入核对声明和信封是否一致，消费者则对所有 clone 集合要求正数。因此只看 producer pass 的调用方可能拿到违反公开协议的产物。**优先修正点**是把公开的下限提前落实到通用生产者。回归验收应覆盖 0 在发布事实前被拒、1 的对照链路通过，并对实际保留的信封运行导出的 Schema 校验。[输出声明](../../src/qualitygate/tool_evidence/contracts.rs) · [协议 Schema](../../src/qualitygate/tool_evidence/schema.rs) · [导入](../../src/qualitygate/tool_evidence/normalization.rs) · [消费](../../src/qualitygate/rule_evaluation/custom_rules/expression_v4.rs)

## Agent 自建工具的两条实测路径

### 工具提供诊断

在独立 Git 仓库中，Agent 写了无依赖 Rust 包 `tools/marker`。它按行扫描 `src/input.txt`，仅当整行等于 `FORBIDDEN` 时输出含 rule、file、line、message 的 diagnostics 条目。正常完成时向 stdout 写 `{"issues":[...]}` 并退出 0，读取失败则返回 I/O 错误。`--version` 输出 `marker 0.1.0`。仓库先提交工具源码、manifest、锁文件、策略及一个基线输入，忽略构建产物，再只改动输入文件。

```yaml
schema_version: 1
rulesets: []
verification_assets: ["tools/marker/**"]
checks:
  - id: marker
    argv: [cargo, run, --offline, --locked, --quiet, --manifest-path, tools/marker/Cargo.toml, --]
    timeout_seconds: 30
    tools:
      - id: marker
        argv: [cargo, run, --offline, --locked, --quiet, --manifest-path, tools/marker/Cargo.toml, --, --version]
        inputs: [tools/marker/src/main.rs, tools/marker/Cargo.toml, tools/marker/Cargo.lock]
    reports: [{format: diagnostics, from_stdout: true, path: marker.json}]
```

Linux x86_64、Rust 1.97.1，开发 CLI 的 SHA-256 为 `a07196d81a1287877033f4c8b6448f95faca722d26c043d6a31070d5d102bab8`。每次运行 `qualitygate --root <fixture> check --worktree --profile full --format json --output-dir <external-dir>`，策略来源为 `local_candidate`，范围为 delivery，三次的待交付检查列表均为空。

| 变更输入 | 工具退出 | check 状态与证据 | Gate 退出 |
| --- | ---: | --- | ---: |
| 一行 `ordinary` | 0 | completed/pass，零诊断 | 0 |
| 追加第二行 `FORBIDDEN` | 0 | completed/fail，一条诊断定位到 `src/input.txt:2` | 1 |
| 删除输入文件 | 1 | tool_error，无 verdict，stdout 报告为空 | 2 |

这里可追溯的可执行入口是 Cargo，探针另绑定检查器源码、manifest 和锁文件。第二组中子进程成功表示扫描完成，Gate 根据诊断中的具体违规返回 1。

### 工具提供事实，V4 规则决定阈值

在第二个隔离仓库中，Agent 编写 `tools/span` Rust 包，读取 `src/lib.rs`，计算一个函数从声明到闭合括号的物理行数。为了给出可手算的独立预期，夹具只支持一个顶层 `pub fn sample() {`，结束行为 `}`，中间每行均为缩进四格的 `// fixture line`。程序从实际输入数行，对超出约定的形态返回错误。

- **生产者** `scan` 沿用上例 Cargo 启动与探针方式，换用 `tools/span/Cargo.toml`，分别绑定该包的源码、manifest 和锁文件。输出改为 `tool_facts`，取 `adapter: tool_facts`、`schema: function_metric`、`from_stdout: true`。从 `context_path: context.json` 读取当次 producer 与 snapshot digest，以 `app::sample` 的实际位置、`span_lines`、单位 `count` 和测量定义 `inclusive-function-lines-v1` 发布事实。
- **消费者** `fixture-span-limit` 使用 V4，显式依赖 `scan`，在 `fact_sources.spans` 中声明相同的项目、指标、单位和定义。断言同时要求 `count(spans) >= 1` 和每项 `value <= 3`。本次只在夹具的本地候选策略中核对了规则与来源条款的映射。[V4 声明示例](../../skills/qualitygate-cli/references/examples/operators-v4.yaml)

| 相同开发 CLI 和执行命令下的输入 | 生产者 | V4 消费者 | Gate 退出 |
| --- | --- | --- | ---: |
| 3 行，尚缺规则来源评审绑定 | pass，值 3 | blocked，提示来源评审缺失或过期 | 2 |
| 3 行，具备夹具来源绑定 | pass，值 3 | pass，包含阈值边界 | 0 |
| 增加一行注释，成为 4 行 | pass，值 4 | fail，违反上限 | 1 |
| 改为不受工具支持的 `pub fn other() {}` | tool_error，无事实 | blocked，无 verdict | 2 |

后三次只改动输入源文件。同一生产者也能供多个阈值共用。仓库的独立回归用已知值 15 验证生产者只运行一次，阈值 10 失败、20 通过，旧快照被拒。[多消费者回归](../../src/qualitygate/gate_execution/fact_workflow_tests.rs)

对下一个项目检查器，先用业务条款写出独立预期的通过、违规和证据不足样例，再让 Agent 实现工具。数值型检查应额外选阈值附近的样例，集合型检查应包含应被检出的实体、容易误认的实体和漏一项的情况。对同一组留出的已标注输入，按检测单位记录真阳性、假阳性、假阴性，另列 incomplete 及其分母。这些数据可用于判断该版本的工具是否适合阻断交付。本次样例只支持上述特定执行与判定结论。
