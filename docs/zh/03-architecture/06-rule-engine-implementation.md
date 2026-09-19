# 规则引擎实现

[English](../../en/03-architecture/06-rule-engine-implementation.md) · [本卷目录](README.md) · [逻辑视图](01-logical-view.md)

目录规则同时具有公开 ID 和 `implementation` ID。公开 ID 保持策略与诊断稳定；implementation ID
从封闭集合中选择一个 Rust evaluator。配置层在执行前拒绝未知参数、错误类型、不支持的语言、非法
glob/正则及不安全组合。适配器创建 pending 结果、验证所需事实、分派 evaluator，再标记 complete
或 blocked。未知实现或能力缺失绝不会变成空通过。

分派实现在 [`adapters/rules.rs`](../../../src/qualitygate/adapters/rules.rs)，参数契约位于
[`config/parameters.rs`](../../../src/qualitygate/config/parameters.rs) 和
[`config/builtin_validation.rs`](../../../src/qualitygate/config/builtin_validation.rs)。

## 渐进精度架构

Qualitygate 使用足以诚实支撑规则的最低成本证据层，同时显式披露层级和边界：

```text
L0  快照与文本信号
    换行、提交元数据、新增文件/新增行模式
                         ↓ 需要更强证据时升级
L1  tree-sitter AST 与规范实体
    测试、注解、import、注释、范围和 base/head 身份
                         ↓ 语法不足时引入生态事实
L2  项目、编译器与原生工具语义
    模块、已解析依赖、bytecode usage、lint/report 证据
```

系统禁止从 L2 静默回退到 L1，或从 L1 回退到 L0。AST 无法解析、必需项目事实或 lint producer
缺失时，检查保持 incomplete。快速文本检查只声明审查信号；声称结构的规则必须消费解析节点，声称
依赖语义的规则必须消费生态证据。这是准确性的核心契约。

当前 tree-sitter 为 Java、Python、TypeScript、Go、Rust 与 Bash/Shell 提供共同结构基础。适配器把
语言树转换成规范测试、注解、注释、import、symbol 和精确 range；base/head 实体匹配再区分新增、
修改、移动与歧义复制。未来可以把更多文本规则迁移为 AST predicate，而不改变 gate/report 协议。
增强准确性应增加经审核 query 与类型化 capability，不能把正则包装成 AST，也不能把 tree-sitter
本身无法提供的类型/数据流事实写进结论。

## 实现类别

| 类别 | Implementation | 技术输入与功能 |
| --- | --- | --- |
| 快照元数据 | `line-ending`、`commit-message`、`diff-size` | 捕获字节、选中提交主题和新增行总数。 |
| 新增文件文本 | `file-pattern`、`shell-shebang`、`shell-commented-code` | 整个新增文件或 Shell 特定行组；`file-pattern` 使用有界并行。 |
| 新增行文本 | `source-pattern` | 只在 diff 新增源码行匹配经审核的正则信号。 |
| 语法实体 | `test-naming`、`test-naming-strict`、`parameterized-tests`、`comment-language`、`ai-code-traceability`、`import-boundary` | tree-sitter 实体及其 base/head 身份。 |
| 项目语义 | `test-annotation-dependency`、`used-undeclared`、`module-boundary` | 绑定快照的 Maven/项目事实与编译使用证据。 |
| 项目 DSL | 自定义规则协议 v1 | 有限且经 Schema 校验的实体、文本、marker、文件清单与依赖断言。 |
| 原生工具 | command check 与报告适配器 | 来自现有工具的覆盖率、SARIF、测试、兼容性和 lint 证据。 |

所有类别输出相同 domain diagnostic：公开规则 ID、稳定 fingerprint、可选文件/范围、消息、结构化证据、
修复建议和复验上下文。fingerprint 对规则、文件和实现特定 identity 做摘要，避免文案变化切断同一问题。

## 快照元数据实现

### `line-ending`

evaluator 遍历 head 快照中每个被选中的变更文件；含 NUL 字节的二进制文件不进入文本契约。已有文件
及重命名文件从对应 base 字节推导预期风格，新文件默认使用 LF。实现按字节区分 LF、CRLF、mixed 和
无换行，只报告两种非空风格之间的变化；它既不改写文件，也不读取可变的 Git 用户配置。

### `commit-message`

一个经验证的正则应用于快照比较选中的每个 commit 的首行。诊断以 commit OID 作为身份，并保留实际
subject 与模式。若比较范围没有新 commit，结果明确标为 skipped，而不是把空清单伪装成已完成检查。
该实现只验证 subject 形状，不证明作者身份或签名。

### `diff-size`

实现对所有选中路径的 `Change.added_lines` 求和，再与正数 `max_added_lines` 阈值比较，只产生一条
汇总诊断。没有新增行的重命名不会放大计数，删除行也不会抵消新增行。这个指标约束评审体量，不度量
语义复杂度。

## 新增文件与 Shell 实现

### `file-pattern`

该实现只选择 change kind 为 `added` 的文件，依次应用语言路由和路径 glob，再扫描每个选中文件的
全部行。显式扩展名映射还覆盖 C/C++。工作通过有界并行 batch 分发；文件缺失或 UTF-8 非法会阻塞
结果，诊断最多 10,000 条。它适用于“新文件不得引入这段文本”，不适用于修改行策略。

### `shell-shebang`

变更文件依据扩展名或可识别的解释器指令路由为 Shell。只有文件为新增，或第 1 行本身属于新增行时，
缺少 `#!` 才产生诊断。这个增量条件避免脚本其他位置变化时追责未触及的历史首行。实现只验证指令
存在，不证明解释器可用或安全。

### `shell-commented-code`

evaluator 把词法上类似赋值、Shell 控制关键字或命令关键字的连续注释分组。连续至少三行且其中至少
一行是新增行时才报告。跨行 block 状态使它不能等价为单行 pattern。它仍是词法启发式：示例代码
可能命中，其他形态的死代码也可能漏掉。

## 新增行实现

### `source-pattern`

`source-pattern` 面向成本低、可审核的信号，例如直接 API 调用、可疑选项或禁用 marker。它明确不声称
具备 AST、类型、控制流或污点语义。

1. 必须配置 `prohibited_patterns`，key 为 `all` 或支持语言。允许 1–32 个语言项，每项 1–32 个
   互不重复的正则，每个 1–512 字节，并在配置验证时编译。可选 `paths` 是仓库相对 glob，
   `languages` 限制路由。带语言前缀的内置规则固定 scope；C-family 显式使用文本 C/C++ 路由。
2. 运行时按顺序遍历快照 change map，应用快照与规则路径过滤，忽略删除文件；其他选中变更文件必须
   存在于捕获快照。
3. 根据路径和支持的 shebang 识别文本源码语言。选中字节必须是 UTF-8；不支持文件被忽略，非法文本
   会阻塞检查，而不是产生空扫描。
4. 合并 `all` 和当前语言正则，按模式字符串去重，确定性读取各行，但只评估
   `Change.added_lines` 中的行号。保留行和删除代码不会产生诊断。
5. 命中记录语言、精确正则和单行 range；identity 来自路径、行号与正则。同一行命中多个模式时可
   有意产生多条独立诊断。
6. 单次评估最多 30 秒、1,000,000 个新增行和 10,000 条诊断。任何限制超出都阻塞为 incomplete，
   不会截断后伪装成通过。

注释、字符串、声明和不可达代码可能命中，多行或间接行为也可能漏报。因此它是审查信号，不是漏洞或
缺陷证明。误报成本较高时，应迁移为 AST entity/query，或复用语义 lint。

## AST 实体实现

实体收集会在路径过滤前解析完整变更的 base/head，依次匹配同路径 symbol、带名称的移动 body、再到
body digest。multiset 算法避免重命名被误判为新增，并标记重复实体歧义。解析共享 deadline，最多
50,000 个测试实体。import 规则只检查与新增行相交的解析范围；注释和测试规则操作类型化 range，
而不是任意文件文本。

### `test-naming`

该实现使用感知测试框架的解析实体，并只评估 base/head 匹配后被归类为新增的实体。它选择配置的
逐语言正则或语言默认值，并把诊断绑定到声明 range。它验证名称，不验证测试有效性或运行时发现。

### `test-naming-strict`

严格实现复用 `test-naming` 的实体抽取与匹配管线，但配置验证把语法 scope 固定为 Java，catalog
提供更严格的命名契约。独立 implementation ID 使固定 scope 可审核，也防止策略通过 language
override 静默扩大范围。

### `parameterized-tests`

新增测试按 enclosing symbol 和归一化 AST shape digest 分组；带已识别参数化注解的实体先排除。
组大小达到 `minimum_similar`（默认 3，最小 2）时产生一条启发式诊断并列出相关测试。形状相等不
证明行为相同，所以该规则提示评审，而不要求自动重写。

### `comment-language`

实现选择 range 与新增行相交的解析 comment node，再应用经审核的豁免正则。English 模式拒绝 CJK
字符；Chinese 模式在没有 CJK 且存在至少三个 ASCII 单词时报告；bilingual 接受两者。这是显式披露
的启发式，不是自然语言分类器。

### `ai-code-traceability`

新增测试以及过去已经承担 trace obligation 的保留测试，会绑定到 annotation、相邻的解析注释或
已验证 Git trailer。必需字段只在引号外解析，示例文字中的赋值不能满足声明。`ai_only` 还要求已
验证的外部 Agent provenance；trailer 模式要求 commit-to-entity 关联。结果证明声明已经绑定，不
证明声明内容真实或代码正确。

### `import-boundary`

配置正则只对 range 与新增行相交的解析 import node 文本运行。AST range 避免命中注释和普通字符串，
并保留多行 import 的结构。该规则执行经审核的源码边界，但不声称具备 resolved dependency 或运行时
数据流语义。

## 项目语义实现

### `test-annotation-dependency`

AST 阶段先选择带指定 annotation 的新增 Java 测试方法；只有确实存在此类测试时，规则才要求 Maven
事实。每个选中文件必须恰好归属于一个与快照绑定的 module，且该 module 必须声明并解析到指定 test
dependency。import 本身永远不能代替 module 证据。

### `used-undeclared`

每个配置 module 必须有且只有一个 Maven producer，并提供同一快照的完整 bytecode usage 清单。每个
已编译使用若缺少直接声明，诊断会保留 scope、artifact type、classifier、analyzer 与 producer 证据。
规则评估完整 module，而不局限于变更行；它不能证明反射或运行时资源形式的依赖使用。

### `module-boundary`

evaluator 建立完整 module inventory，选择 declared 或 resolved edge，去重重复的传递路径，再以经
审核的 `from`、`to` 和可选 scope glob 检查每条 edge。一条关系只产生一条诊断，并列出所有命中的
禁止方向。重复身份、外来快照事实、不支持的 Schema 或缺失 manifest 都会阻塞评估。

项目规则不会从 import 拼写推断依赖语义。每个 module 都要求唯一显式 producer、匹配的 snapshot
digest，以及位于被检查快照内的 manifest。

## 自定义规则实现

自定义协议 v1 是解释执行的数据，不是可执行插件代码。它从 commit、file、test-method、comment 或
import subject 中选择对象，再应用封闭集合内的 name/text、count、marker、dependency、required-path、
line 与 word assertion。实体变化复用内置 AST multiset matching。配置 `change: all` 的 file 规则
会纳入未变文件，但受 30 秒、50,000 文件与 32 MiB 选中文本预算约束。AI provenance、Git trailer
及依赖断言都要求显式 producer；能力缺失是 incomplete，不是“没有匹配”。

## 外部 lint 与报告实现

外部 lint 不是另一个 catalog implementation 字符串，而是有界 command check 加类型化 report
adapter。runner 固定 argv、cwd、环境策略、deadline、输出上限、允许的 status 和工件路径。adapter
验证 producer 完成状态，并归一化原生 rule ID、location、count、version 与原始报告 digest。ratchet
模式会在 base/head 上运行同一个 producer，再比较计数或稳定身份。

## 复用 lint，而不是过度包装

Qualitygate 不应重新实现 Clippy、ESLint、golangci-lint、Ruff、Checkstyle、PMD、SpotBugs、GCC/Clang
分析器或兼容性工具。成熟工具拥有语义时，command check 在两个不可变快照内运行原工具，报告适配器
保留其原生 rule ID、位置、版本、完成标志和原报告摘要。ratchet 模式比较基线/当前计数或稳定身份，
不维护可编辑的“债务文件”。

Qualitygate 增加的是单个 lint 通常不负责的编排：Git 快照身份、策略/任务组合、有界执行、报告完整性、
跨工具 gate 语义、可复验证据和 Agent 复验反馈。如果只改名 lint 消息，或用正则近似现成语义规则，
就是过度包装。只有需要快照/diff 语义、小型可移植约定、跨语言一致性，或配置工具确实缺少能力时，
内置实现才合理。

因此扩展路径保持清晰：已有 lint 格式新增 parser/adapter；结构准确性新增 AST query；文本与 AST 都
无法证明的事实才进入项目/编译器适配器。策略可组合三层能力，但不会混淆其证据强度。

## 验证证据

[`adapters/rules_tests.rs`](../../../src/qualitygate/adapters/rules_tests.rs) 的聚焦测试证明
`source-pattern` 只报告新增行并阻塞非法配置。各 Issue 集成目标覆盖路由、固定规则包 scope、修复、
非法 UTF-8、文件缺失、预算和确定性顺序。结构、项目、自定义规则与报告测试分别覆盖移动、歧义、
快照不匹配、事实缺失、超时和工具输出错误。
