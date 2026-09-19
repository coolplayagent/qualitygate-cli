# qualitygate-cli 需求文档

> 版本：v0.2（草案）
> 状态：待评审
> 关联讨论：把 AGENTS.md / 团队规范从“提示词记忆”变成“可执行检查器”，作为 AI 编程 harness 的规范执行与验收层

## 1. 项目定位

qualitygate-cli 是 **AI 编程 harness 的规范执行与验收层**：将团队文档（AGENTS.md、开发规范、提交规范等）中的可判定约定转化为检查规则，结合本次任务的验收契约和现有构建、测试工具，输出可复验的结果与门禁结论。

**核心价值**：让 agent 在交付前发现并修复问题，让开发者与 CI 使用同一套验收规则，减少规范遗漏和人工检视返工。

### 1.1 在 harness 中的职责

```text
团队规范 + 任务验收契约
          ↓
生成检查计划 → agent 修改代码 → qualitygate 检查
                                  ↓
                         结构化诊断与执行证据
                                  ↓
                         agent 修复 → 重新检查
                                  ↓
                      必需检查完成、阻塞条件满足
                                  ↓
                       外层 harness 判定能否交付
```

qualitygate 负责检查计划、规则执行、验证命令运行、结果聚合和门禁判定。外层 harness 负责模型调用、任务拆分、上下文与进度管理、修复调度、重试预算和最终交付。

### 1.2 能力边界

- 仓库规范合规与任务完成分别判定；命名、注释、标记合规不能证明业务功能正确。报告必须说明覆盖了哪些验收项。
- 初期由团队维护“规范 → 规则”的映射，`init` 生成配置建议；不承诺自动、完整地理解任意自然语言规范。
- 复用现有编译器、测试框架和静态检查工具。自研规则优先覆盖团队反复遇到、现有工具未覆盖的约定。
- 统一配置和结果协议，逐步扩展已验证的生态能力；不承诺所有语言仅靠 YAML 和语法 query 即可支持。
- AI 来源标记用于检查声明完整性；真实执行来源的审计需要外部运行记录，见 §6.4。

## 2. 设计原则

| 原则 | 说明 |
|---|---|
| **增量视角** | 区分检查范围与阻塞依据，明确新增诊断、变更行和受影响范围的语义，见 §3.5 |
| **规则声明式** | YAML 配置规则参数、适用范围和策略；复杂语义由适配器或生态工具实现 |
| **分级输出** | error（必须修）/ warning（建议修）/ info（提示），附修复建议 |
| **完整性优先** | “没有违规”与“必需检查已完成”分别判定；检查缺失不能被解释为通过 |
| **可进流水线** | exit code：0=门禁通过，1=存在阻塞违规，2=无法完成门禁判定，详见 §6.3 |
| **显式适用性** | 引擎不强加团队规范；明确不适用才允许 skipped，必需检查缺少能力或证据必须阻塞 |
| **证据绑定** | 结果绑定代码快照、规则与配置版本、实际执行记录；代码或策略变化后重新验证 |
| **生态解耦** | 文件语法分析与项目语义分析分别适配；能力不足时明确报告 |
| **策略可追溯** | 规则关联规范来源，CI 使用受信策略；配置变化不能静默降低验收要求 |

## 3. 功能需求

### 3.1 变更规范检查（diff 级，核心）

| 检查项 | 检测逻辑 | 说明 |
|---|---|---|
| AI 来源声明完整性 | 团队指定范围内的新增测试是否带约定标记（绑定见 §6.4） | 不根据代码风格猜测 AI 来源 |
| 配套依赖存在性 | 已要求使用或实际使用标记，但配套依赖缺失 → error | 如 Java 生态的 aitool；缺失不能使强制规则跳过 |
| 测试命名规范 | 新增测试是否匹配团队与框架约定，如 Java 的 `should_[ExpectedBehavior]_when_[Condition]` | Python、Go 等保留框架所需前缀，不套用同一命名模式 |
| 参数化测试建议 | 新增多个结构相似的独立测试时提示评估参数化 | 启发式规则，阈值可配；完成误报评估前不作为阻塞项 |
| 注释语言 | 新增注释是否符合团队语言约定（中文/英文/双语） | 明确术语、代码片段等豁免；不确定时提供建议 |
| 换行符保持 | 修改文件行尾与原文一致（LF/CRLF），新文件默认 LF | |
| 提交信息格式 | 匹配 `[ID]type: description` 风格（ID/type 集合可配置） | |
| 新依赖声明检查 | 由生态工具解析导入归属、模块边界与依赖清单，确认未声明后报 error | 不能仅按 import 文本猜测；不支持解析时报告能力缺失 |
| 模块依赖边界 | 变更违反团队约定的模块访问或依赖方向 → error | 优先接入现有架构检查工具 |

Issue 9 扩展四个默认不启用的可配置内置规则：`commit-message-convention` 校验新增提交主题，`test-naming-strict` 校验新增 Java 测试方法名，`test-annotation-dependency` 仅对带指定注解的新增 Java 测试方法要求指定 Maven 依赖的声明及解析事实，`no-hardcoded-secrets` 检查新增 Java 文件中的字面凭据赋值。四者默认级别为 error；缺失或失效的必需事实、语法解析失败及预算超限均报告未完成。大仓库分析使用有界并行、固定截止时间和有序结果。需求到测试证据见 [Issue 9 验收映射](docs/acceptance-evidence.md#issue-9-reusable-built-ins)。

Issue 10 扩展三个默认不启用、默认级别为 error 的新增文件规则：`no-printf-log` 和 `no-unsafe-string` 检查 C/C++ 源码中的调用文本模式，`no-test-sleep` 在可配置的测试路径中检查跨语言休眠调用文本模式。语言范围固定，路径和正则可配置；这些检测属于逐行文本信号，注释、字符串或声明可能匹配，不宣称语义解析。选中文件缺失、非 UTF-8、配置失效或预算超限报告未完成。需求到测试证据见 [Issue 10 验收映射](docs/acceptance-evidence.md#issue-10-native-and-test-file-built-ins)。

Issue 11 扩展六个默认不启用的内置规则：`no-bare-except`、`no-os-path` 和默认 warning 的 `no-print` 检查新增 Python 文件，`no-emoji` 检查新增 UTF-8 文件的配置 Unicode 范围，`commit-message-format` 校验新增提交主题，`python-test-naming` 校验新增 pytest 可发现测试方法名。默认语言范围固定，路径或正则可按仓库政策调整。文件模式是逐行文本信号，`no-emoji` 不检查提交信息，也不涵盖全部 emoji 序列；PEP 8、PEP 428 和 Conventional Commits 并不强制这些更严格的项目约定。选中文件不可读取、语法错误、配置无效或预算超限仍报告未完成。需求到测试证据见 [Issue 11 验收映射](docs/acceptance-evidence.md#issue-11-python-and-neutral-built-ins)。

Issue 12 扩展十八个默认不启用的 Shell 内置规则，涵盖凭据、调试输出、SQL 插值、赋值和比较空格、重定向、信号处理、shebang、临时路径及连续注释代码。十六项针对新增行使用可配置文本正则；`shell-missing-shebang` 检查新增脚本或变更后的首行，`shell-commented-dead-code` 检查含新增行的连续三行代码式注释。语言范围固定为 `shell`，识别 `.sh`、`.bash` 及带可识别 Shell shebang 的无扩展名文件；无法从无扩展名且无 shebang 的文件推断 Shell 类型。文本规则是需人工评估的信号，不宣称证实秘密泄漏、SQL 注入、密码算法用途或 Shell 运行结果。超出 30 秒、10000 条诊断、1000000 行范围以及选中输入失效均报告未完成。需求到测试证据见 [Issue 12 验收映射](docs/acceptance-evidence.md#issue-12-shell-built-ins)。

Issue 13 通过有界 stdout 报告和 Cargo JSON Lines 解析支持真实 Clippy 诊断数量棘轮：两个不可变快照分别运行相同工具，按 `(tool, rule)` 计数；缺失完成记录、编译错误、失效路径或版本不一致报告未完成。另增三个默认不启用的 Rust 单行文本审查规则，分别提示未显式声明 ABI 的 extern 块、动态库非字面量路径、同一行宏定义中的 unsafe 块。文本信号不能证明攻击者输入或语义安全。需求到测试证据见 [Issue 13 验收映射](docs/acceptance-evidence.md#issue-13-clippy-ratchet-and-rust-rules)。

Issue 14 通过 ESLint JSON 文件结果适配和两个快照中的真实工具执行支持诊断数量棘轮，保留版本、命令、原始报告及源位置证据。空文件清单、致命解析错误、计数矛盾、路径无法映射或不认可的退出码报告未完成。另增十二个默认不启用的 TypeScript/JavaScript 新增行文本审查规则，需显式选择 `lang-typescript` 规则包；它们不证明注入路径、随机数安全用途、消息目标安全或个人信息分类。需求到测试证据见 [Issue 14 验收映射](docs/acceptance-evidence.md#issue-14-eslint-ratchet-and-typescript-rules)。

Issue 15 通过 golangci-lint v2 JSON 报告适配和两个不可变快照中的真实工具执行支持 Go 诊断数量棘轮。必须禁用生产者的数量截断、逐行去重和 stdout 统计摘要；分析错误、警告、编译类型错误、缺失位置或不认可的退出码报告未完成。另增十二个默认不启用的 Go 新增行文本审查规则，需显式选择 `lang-go` 包。这些规则只提供同一行文本信号，不证明输入污染、凭据有效性、函数可见性、CGO 释放配对或实际日志敏感性。需求到测试证据见 [Issue 15 验收映射](docs/acceptance-evidence.md#issue-15-golangci-lint-ratchet-and-go-rules)。

Issue 16 通过 Ruff JSON 报告适配和两个不可变快照中的真实 Ruff 执行支持 Python 规则数量棘轮。配置错误、解析失败、无效位置或不认可的退出码均报告未完成；Ruff JSON 没有已检查文件清单，空数组须结合明确目标路径审查。另增十三个默认不启用的 Python 新增行文本审查规则，需显式选择 `lang-python` 包；调用、参数与变量名模式不证明污点来源、凭据有效性、安全 Loader、生产环境用途或日志敏感性。需求到测试证据见 [Issue 16 验收映射](docs/acceptance-evidence.md#issue-16-ruff-ratchet-and-python-rules)。

Issue 18 扩展七个默认不启用的 C/C++ 新增行审查规则，需显式选择 `lang-c` 包；文本匹配仅标记变长数组声明、断言副作用、无条件循环或浮点循环计数、`sizeof` 副作用、旧式临时文件名 API、终止或分配 API、易混淆的小写 `l` 后缀。不能据此判断越界、空指针、控制流完整性、权限、路径污染或符合 CERT 标准。外部 C/C++ 分析器可用已有 SARIF `ratchet` 模式在不可变基线和当前快照中分别执行；源位置、失败退出码、缺失及格式错误报告按现有契约报告未完成。需求到测试证据见 [Issue 18 验收映射](docs/acceptance-evidence.md#issue-18-c-family-rules-and-analyzer-ratchet)。

Issue 19 增加九条默认不启用的 C++ 新增行审查规则和 Clang 静态分析器 SARIF 棘轮参考配置，须显式选择 `lang-cpp` 包及规则。正则仅标记可直接观察的调用、异常语句、锁方法和返回表达式，不证明指针大小、对象生命周期、竞态、随机数安全用途或异常传播；其余提案需要语义分析。棘轮基线始终由不可变 Git 快照重新执行产生，不建立可编辑的债务文件；数量和变化已在报告元数据中显示。需求到测试证据见 [Issue 19 验收映射](docs/acceptance-evidence.md#issue-19-cpp-rules-and-ratchet-adoption)。

Issue 20 增加十二条默认不启用的 Java 新增行文本审查规则，固定 `languages: [java]`，聚焦线程、资源、编码、随机数、弱算法、异常及直接 SQL/命令调用的可见模式。它们不能证实污点、控制流、类型或运行时安全。Checkstyle、PMD、SpotBugs XML 棘轮参考配置使用已有不可变快照和 `(tool, rule)` 计数；无效输出、缺少编译输入或不认可的退出码报告未完成。未纳入内置规则的语义提案和类别棘轮边界见 [Java 规则评估](docs/java-rule-triage.md)，需求到测试证据见 [Issue 20 验收映射](docs/acceptance-evidence.md#issue-20-java-rules-and-analyzer-ratchet-references)。

规则优先级由试点仓库的历史违规、影响和检测可靠性决定。AI 标记、注释语言、相似测试建议不作为所有仓库默认启用的强制规范。

### 3.2 构建 / 静态质量聚合

| 检查项 | 检测逻辑 | 说明 |
|---|---|---|
| 构建命令规范 | 检查配置中的命令是否带必需参数（如 Maven `-s settings.xml`）；执行时记录实际参数和结果 | 脚本中出现命令不能作为已运行的证据 |
| 编译 / 测试 / 构建 | 运行仓库已有命令，采集退出码、耗时、日志与报告 | 配置工作目录、超时、前置步骤；区分测试失败与工具故障 |
| 静态检查增量过滤 | 接入 checkstyle / spotbugs / pmd / CodeCheck 等结果，按规则配置的新诊断或变更范围过滤 | 不能用“改动文件全部告警”冒充新增问题，详见 §3.5 |
| 覆盖率增量 | 接入行 / 分支覆盖率工具，按配置阈值判定变更代码覆盖情况 | 缺少匹配快照的报告不能算通过；100% 覆盖不能证明行为正确 |
| 接口兼容性 | 接入已有兼容性工具，对比基准版本与当前版本 | 是否必需由仓库策略或任务契约声明 |

外部报告必须提供可校验的代码快照、生成命令和工具版本关联。无法验证来源、报告格式错误或必需报告缺失时，该检查不能标记为通过。

### 3.3 流程门禁

| 功能 | 说明 |
|---|---|
| 提交前验证清单 | 按配置依赖关系执行验证命令并聚合结果；前置步骤失败时明确列出后续未执行项 |
| 分阶段检查 | `quick` 提供修改后的快速反馈，`full` 执行交付所需检查；快速检查报告只证明自身覆盖范围 |
| 任务验收契约 | 声明本次任务的验收项、验证方式和必需检查，与仓库策略合并，不能降低仓库要求 |
| agent 修复反馈 | 输出稳定诊断标识、证据位置和复验命令，供外层 harness 驱动修复与重跑 |
| MR 预检报告 | 对 MR 输出规范检查报告，作为检视前置/检视意见素材 |
| CI 门禁 | 验证报告对象与待验收代码一致，必需检查完整，且受信策略未被绕过 |

### 3.4 任务验收契约与修复闭环

任务契约与仓库规则分开维护，至少包含任务标识、验收项标识、可观察的预期行为、验证方式和关联检查。以下为配置草案示例：

```yaml
schema_version: 1
task_id: order-idempotency
acceptance:
  - id: duplicate-request
    description: 同一业务请求重复提交时只创建一笔订单
    verification:
      kind: command
      check_id: order-idempotency-test
      argv: ["./mvnw", "-s", "settings.xml", "-Dtest=OrderIdempotencyTest", "test"]
      cwd: .
      timeout_seconds: 300
    required: true
    severity: error
```

运行过程必须满足：

1. 合并受信仓库策略和任务契约，生成检查计划，列出适用范围、必需项、执行顺序及验收条件。验收条件包括预期退出码和必要的报告要求；测试检查应确认目标测试实际执行，不能把“零测试执行”当作测试通过。
2. 对确定的代码快照执行检查，采集结构化诊断和执行证据。代码在执行期间变化时，受影响结果失效。
3. 外层 harness 根据诊断修复代码，再运行相应检查；交付前对最终快照完成全部必需检查。修复建议不能默认要求关闭强制规则或降低严重级别。
4. 必需项完成且无阻塞违规后，输出通过结论；结果只证明契约中已验证的内容。未提供任务契约时，报告标明仅覆盖仓库策略，不能据此宣称任务完成。
5. 人工验收项必须关联外部可信验收记录；没有记录时保持未完成。M1 支持命令式验收，人工记录接入在后续阶段实现。

任务契约的建立和变更也需要由外层流程确认；agent 自行删除验收项不能使原有交付要求消失。重试次数、费用预算、连续无进展后的处理由外层 harness 管理。
总耗时预算在初始检查或代理启动前耗尽时，外层 harness 必须记录 `time_budget` 和已采集的日志，不得把没有 JSON 输出的超时误记为普通执行错误；代理已启动后的超时仍保留该次尝试的证据。

### 3.5 增量检查语义

**检查范围可以大于 diff；报告过滤与门禁阻塞依据必须分别声明。** 每条检查记录所用模式、基准快照和过滤统计。

| 模式 | 定义 | 使用边界 |
|---|---|---|
| `new_diagnostics` | 对比基准与当前诊断，按规则和稳定指纹匹配，报告新出现的问题 | 用于支持基准对比的静态工具；位置移动不应自动算新增 |
| `changed_lines` | 将诊断位置映射到变更行，仅保留匹配问题 | 适用于局部规则；报告注明无法覆盖变更行外的影响 |
| `affected_scope` | 分析变更影响的符号、模块或测试范围，检查范围可包含未修改文件 | 由生态工具提供影响信息；是否阻塞依据规则与基准策略 |
| `full` | 完整执行指定检查并使用其验收结果 | 适用于构建、必需测试等；不承诺排除历史失败 |

- 修改文件内的历史告警不能仅因该文件进入 diff 而被认定为新增问题。方法重命名、移动与删除需要基准映射，不能只用行号或 `is_added` 判定。
- 基准缺失、不可解析或不可比较时，必须报告限制；必需的增量检查保持未完成，或使用受信策略事先允许的完整检查回退并明确披露，不能直接视为通过。
- `--diff <base>..<head>` 比较两个提交快照；MR 模式使用目标分支与源分支的 merge-base 作为基准，并记录实际提交标识。
- `--staged` 读取暂存区内容；`--worktree` 读取当前工作区及范围内的未跟踪文件。不得用工作区源码解析结果冒充暂存区结果。
- 选择 `--path` 或 `quick` 后被省略的交付检查需列为未执行；局部通过不能替代 `full` 门禁。

## 4. 通用化架构：三层解耦

```
┌─────────────────────────────────────────────────┐
│  L0 核心引擎（语言无关）                          │
│  快照 / 检查计划 / 执行调度 / 报告 / CI 门禁      │
├─────────────────────────────────────────────────┤
│  L1 事实与工具适配器                             │
│  源码语法 / Git 与文件事实 / 项目语义 / 工具结果  │
│  tree-sitter / 构建与依赖工具 / 测试与静态工具    │
├─────────────────────────────────────────────────┤
│  L2 规则与验收层（配置 + 生态包）                 │
│  结构规则 / 语义规则 / 仓库策略 / 任务契约        │
│  core（通用）/ lang-java / lang-python / ...     │
└─────────────────────────────────────────────────┘
```

核心引擎通过统一协议消费事实与检查结果，语言细节封装在适配器中。语法结构、Git 元数据、原始文件属性和项目语义分别提供，不强行塞入单一源码结构视图。

YAML 声明规则选择、参数和策略；复杂判定允许使用生态工具与适配器代码。新增生态优先复用协议，确需扩展时进行版本化演进，不承诺引擎永远零改动。

## 5. 多语言支持

### 5.1 统一结构视图（Canonical Structure View）

结构视图统一可共享的概念，同时保留语言、框架、模块和能力信息。相似语法概念不意味着语义和检测成本相同：

| 概念 | Java | Python | TypeScript | Go | Shell |
|---|---|---|---|---|---|
| 测试方法 | `@Test void x()` | `def test_x()` | `it()/test()` | `TestXxx` | — |
| 注解/标记 | `@AIGenerated` | `@pytest.mark...` | 注释或框架约定 | 注释 | 注释 |
| 依赖声明 | pom.xml | pyproject/requirements | package.json | go.mod | — |
| 导入/加载语法 | `import` | `import` | `import` | `import` | `source`，不等同于包依赖 |
| 注释 | `//` `/* */` | `#` | `//` `/* */` | `//` `/* */` | `#` |

语法适配器输出示例；`change` 由基准与当前结构比较补充，依赖是否已声明由项目语义适配器另行提供：

```json
{
  "language": "java",
  "framework": "junit5",
  "module": "orders",
  "capabilities": ["test_methods", "annotations", "comments"],
  "tests": [
    {
      "symbol": "OrderTest#should_create_when_valid",
      "name": "should_create_when_valid",
      "range": {"start_line": 42, "end_line": 49},
      "annotations": ["AIGenerated"],
      "change": "added"
    }
  ],
  "imports": [],
  "comments": []
}
```

### 5.2 适配器职责与接口草案

```rust
trait LangAdapter {
    fn detect(&self, file: &SnapshotFile) -> Option<LanguageMatch>;
    fn capabilities(&self) -> Vec<Capability>;
    fn parse(&self, file: &SnapshotFile) -> Result<FileStructure, AdapterError>;
}

trait ProjectAdapter {
    fn detect(&self, snapshot: &Snapshot) -> Vec<ProjectContext>;
    fn analyze(&self, project: &ProjectContext, snapshot: &Snapshot)
        -> Result<ProjectFacts, AdapterError>;
}
```

接口展示职责划分，具体类型在实现阶段确定。`SnapshotFile` 必须包含指定快照的内容，不能只传路径后隐式读取当前工作区。

- **语法适配器**：优先用 tree-sitter 和语言 query 提取测试、注解、注释等结构，报告解析错误与不支持的语法。
- **项目语义适配器**：结合构建配置、模块关系和生态工具解析依赖归属、符号关系与框架语义。不能仅从单文件 import 推导 `used_undeclared`。
- **工具结果适配器**：将已有构建、测试、覆盖率和静态检查结果转为统一诊断与证据，声明报告格式及版本支持范围。

tree-sitter 提供语法树能力，不自动提供类型解析、包坐标映射或跨模块依赖分析。缺少必需能力时使用 §6.3 的未完成语义。

### 5.3 多语言仓库

混合语言仓库（如 Java + Shell + Python）先按文件语言路由语法检查，再按项目 / 模块路由语义检查与验证命令。跨模块影响不能仅按变更文件语言隔离。

对未知语言或缺少适配器的项目，报告检测结果与能力缺口；已配置的必需检查不能因此自动跳过。第二种语言通过真实仓库验证后，再扩展支持矩阵。

## 6. 规则系统设计

### 6.1 规则分层

| 层 | 定义 | 例子 | 复用性 |
|---|---|---|---|
| **L0 通用规则** | 与语言完全无关 | 提交信息格式、换行符保持、diff 规模告警 | 所有仓库直接用 |
| **L1 结构规则** | 消费适配器提供的源码结构 | 测试命名、标记完整性、注释语言 | 在支持相应能力的语言中复用 |
| **L2 生态规则** | 依赖项目或生态事实与工具结果 | 依赖声明、模块边界、Java aitool 配套、pytest marker 配置 | 按生态打包，允许适配器代码 |

生态包文件布局：

```
skills/qualitygate-cli/references/rules/
  core/          commit-message.yaml, diff-size.yaml, line-ending.yaml
  shared/        test-naming.yaml, comment-language.yaml, ai-code-traceability.yaml,
                 parameterized-tests.yaml, security-sensitive-api.yaml,
                 todo-marker.yaml, import-boundary.yaml
  lang-java/     junit-naming.yaml, module-boundary.yaml, used-undeclared.yaml
  lang-python/   pytest-naming.yaml
```

### 6.2 规则配置与 DSL 演进

M1 使用有限的内置规则与命令检查配置；通过第二种语言验证公共概念后再开放自定义 DSL。以下为目标配置草案，不代表已经稳定的接口。示例中团队明确要求所有新增测试携带来源声明；若只要求 AI 参与的改动，需要额外的可信范围信息，见 §6.4。

```yaml
# lang-java/ai-code-traceability.yaml
id: ai-code-traceability
version: 1
source:
  document: AGENTS.md
  section: 测试代码来源声明
  content_hash: "sha256:<关联规范章节摘要>"
language: [java]
required: true
severity: error
applies_to:
  paths: ["**/src/test/**/*.java"]
  provenance_scope: all_added_tests
requires_capabilities: [test_methods, annotations, dependency_resolution]
binding:
  marker:
    type: annotation
    name: AIGenerated
    fields: [author, date, description]
when:
  entity: test_method
  change: added
then:
  require_marker: true
  require_dependency:
    group: com.huawei.dfv.s3.common
    artifact: aitool
fix: "按实际来源补齐约定字段，并声明配套依赖，然后重新检查"
```

`requires_capabilities` 描述运行检查所需的能力；`then` 描述被检查对象必须满足的条件。被要求存在的依赖属于验收条件，不能作为缺失后自动跳过规则的先决条件。必需规则的绑定方式由受信策略确定，执行时不得自动降级为更宽松的替代绑定。

### 6.3 适用性、执行状态与门禁语义

每个检查分别记录以下维度，避免把规则违规、工具故障和不适用混为一谈：

| 维度 | 值 | 语义 |
|---|---|---|
| `applicability` | `applicable` / `not_applicable` / `unknown` | 依据受信策略和当前检查对象判定；无法识别不等于不适用 |
| `execution.status` | `completed` / `not_run` / `blocked` / `timed_out` / `tool_error` | 是否运行并取得有效结果；测试断言失败通常仍是 completed |
| `verdict` | `pass` / `fail` / `skipped` / `null` | 已完成检查给出 pass/fail；明确不适用给出 skipped；无法完成判定为 null |

明确不适用时，组合为 `not_applicable + not_run + skipped`，必须给出原因及判断依据。能力缺失、解析失败、前置命令失败、超时、报告缺失和未知适用性都不能用 skipped 隐藏。已正确解析且没有匹配实体时，可报告 `completed + pass`，同时记录匹配数量为零。

`required` 控制检查完整性要求，`severity` 控制违规是否阻塞：必需检查未完成会阻塞，即使其违规级别是 warning；已完成检查的 warning / info 不阻塞，error 阻塞。非必需检查无法执行时仍完整披露，但不阻塞其他已满足的门禁条件。

| 门禁字段 / 结论 | 语义 |
|---|---|
| `complete` | 所选验收范围内的必需检查已完成或有明确的不适用依据，且证据与策略有效；不表示没有违规 |
| `decision: pass` | 门禁完整，且没有 error 违规及其他阻塞条件 |
| `decision: fail` | 门禁完整，但存在阻塞违规 |
| `decision: incomplete` | 必需项未完成或其适用性未知、门禁所需证据失效、策略 / 配置无效，无法完成门禁判定 |

空检查计划或没有任何检查取得可判定结果时（例如全部不适用，或全部可选检查均无法执行），返回 `incomplete` 并说明未覆盖有效检查；不得展示为已验证通过。没有配置来源声明规范的仓库，其相关规则可以 skipped，但仓库整体结论仍由实际检查覆盖情况决定。

退出码固定为：`0=pass`，`1=fail`，`2=incomplete`。同时存在阻塞违规和导致门禁未完成的条件时，返回 2，保留全部违规。输出格式和严重级别显示过滤不得改变门禁计算；无需额外传 `--exit-on-error` 才能阻塞 CI。

### 6.4 AI 来源声明与执行溯源

该能力按团队需求启用，规则标识保留 `ai-code-traceability`。需区分“存在来源声明”和“有执行证据支持该来源”，不能将二者混称为已证明 AI 生成。

| 方案 | 可验证内容 | 限制 |
|---|---|---|
| 代码内注解 | 标记与字段是否满足约定，配套依赖是否存在 | 不能仅凭注解证明真实来源 |
| 注释标记 | 目标代码范围内是否存在约定声明 | 需明确关联范围，不能用无关注释代替 |
| git trailer | 提交是否包含约定声明 | 属于提交级信息；未提交变更不能借用旧提交的 trailer |
| git author / blame / 分支 | 提交者与代码变更线索 | 作为辅助信息，不能可靠区分人写与 AI 生成 |
| 外部 agent 运行记录 | 可信记录中的 run ID、输入 / 输出快照与改动范围是否匹配 | 依赖外层 harness 提供可校验的记录，后续阶段接入 |
| 无标记规范 | 明确说明团队未要求该检查 | 相关规则 skipped |

团队必须明确标记适用范围：所有新增测试，或由外部可信运行记录确定的 AI 参与变更。后一种范围不能靠“是否已有标记”反推，否则漏标代码永远不会被检查；范围证据缺失时，必需检查未完成。

标记形式由团队策略绑定。`init` 可以建议注解、注释或 trailer 方案，运行检查时不能因缺少注解包而静默替换绑定，更不能自动关闭必需检查。

### 6.5 配置自动发现（init）

```
qualitygate init 扫描仓库:
  ├─ 语言/生态识别（pom.xml / package.json / pyproject.toml / go.mod / *.sh）
  ├─ 输出可用生态包、检查能力及缺口
  ├─ 结合仓库现状建议标记绑定和现有验证命令
  └─ 生成 qualitygate.yaml 草案（团队维护并纳入策略管理）
```

```yaml
# qualitygate.yaml（试点配置草案）
schema_version: 1
languages: [java]
rulesets:
  - core
  - lang-java
rules:
  ai-code-traceability:
    enabled: true
    required: true
    severity: error
  test-naming:
    enabled: true
    required: true
    severity: warning
checks:
  - id: repository-verify
    kind: command
    argv: ["./mvnw", "-s", "settings.xml", "verify"]
    cwd: .
    timeout_seconds: 600
    required: true
    severity: error
profiles:
  quick:
    include: [ai-code-traceability, test-naming]
  full:
    include: [ai-code-traceability, test-naming, repository-verify]
# 项目规则可通过 custom_rules 指向 qualitygate/rules/ 或其他项目内目录。
```

生态探测结果不能自行决定团队必须采用某种规范。已有配置不应在重复 `init` 时被静默覆盖。

### 6.6 规范映射与策略变更

- 每条规则记录稳定 ID、规则版本、规范来源文档 / 章节及内容摘要。团队维护可执行条件与正反例；自然语言中的模糊要求先澄清，再决定是否进入强制门禁。
- 关联规范章节变化后标记规则待复核，记录规则是否同步更新；必需规则未完成同步复核时，门禁保持未完成。不能仅修改摘要就声称完成复核，需在受信策略版本中保留复核记录。
- CI 从受保护基准或外部配置取得受信策略，记录实际来源与摘要。新增、删除、禁用规则，降低严重级别，修改适用范围、绑定、必需检查或任务契约，都作为策略差异单独呈现。
- 当前变更不能仅通过修改 `qualitygate.yaml`、验收契约或测试脚本就取消既有要求；策略和验证资产的变更按团队既有评审流程确认。工具负责发现差异并校验受信来源，审批由外层流程执行。
- `rules enable` 等命令修改本地候选配置；其变化须进入上述策略流程。初次接入仓库先建立受信配置，再启用强制门禁。
- 规则或契约摘要改变后，旧报告不能用于新策略。报告摘要用于版本关联，不能替代 CI 对报告来源与实际执行的验证。

## 7. 命令面设计

```bash
qualitygate init                                  # 探测能力，生成配置草案
qualitygate check --diff <base>..<head>             # 比较提交快照
qualitygate check --worktree --base <base>         # 检查工作区相对基准的变更
qualitygate check --path <file>                    # 工作区指定文件的局部检查
qualitygate check --staged                         # 检查暂存区相对 HEAD 的变更
qualitygate check --mr <url>                       # MR 模式，后续阶段接入
qualitygate check --worktree --profile quick       # agent 修改后的快速反馈
qualitygate check --worktree --profile full --task qualitygate-task.yaml
qualitygate rules list                            # 列出规则、来源、能力与启用状态
qualitygate rules enable <rule-id>                 # 修改本地候选配置
qualitygate config --show                         # 展示生效配置与来源
# 通用: --format json|table|markdown  --severity error
# CI: --policy-ref <trusted-ref>，由 CI 提供并校验受信策略引用
```

`--diff`、`--worktree`、`--staged`、`--mr` 是互斥快照选择器；未指定对象时默认工作区。`--path` 是可组合的文件或目录反馈过滤器，单独使用时选择工作区；过滤不能删除策略、基线或构建依赖。`--base` 用于工作区，未指定基准时默认为 HEAD，无有效基准时明确报错。默认 profile 为 `full`；`quick` 报告标识自身范围，并列出尚未完成的交付检查。

Issue #2：18,000 个文件、总内容超过 32 MiB、变更小于 10 KiB 的仓库必须支持上述本地选择器，不能因为把整个树放进一次 `cat-file` 输出而触发 16 MiB 限制。获取内容前验证对象大小，每批最多 4 MiB 内容和 1,024 个对象；工作区每批最多 64 个文件。基础树和目标树共享 1–16 个内容读取任务（默认 4），结果顺序及摘要不依赖调度。每树默认 256 MiB、上限可配置至 1 GiB，文件数量上限 100,000，单文件仍为 2 MiB；每次获取默认 120 秒并有显式超时。测试必须验证串行/并行结果一致、预算超限及不完整执行；18,000 文件的本地快照获取回归阈值为 60 秒，CLI quick 单次为 120 秒。性能测试是受控夹具证据，不代替用户硬件上的测量。

`full` 计划必须包含受信仓库策略和 §3.4 任务契约的全部必需项，配置遗漏时按无效配置处理。`quick` 或 `--path` 的退出码只描述当前检查范围；CI 交付门禁必须校验完整范围、`full` profile 及预期任务契约，不能只看退出码 0。

`--severity` 仅过滤诊断展示，JSON 始终保留完整计划、门禁结论和过滤数量。`--policy-ref` 的可信性由 CI 验证，不能因为本地传入一个引用就宣称受信。

## 8. 输出格式

### 8.1 报告协议

报告区分运行、检查和诊断，避免将“一条规则”与“多个问题”混计。JSON 包含协议版本；字段语义变更需版本化。以下为只有一个检查的简化示例，摘要占位符不表示真实运行证据：

```json
{
  "schema_version": 1,
  "run_id": "qg-example-001",
  "scope": "repository",
  "profile": "full",
  "snapshot": {
    "mode": "worktree",
    "base": "<base-commit>",
    "head": "<head-commit>",
    "content_digest": "sha256:<checked-content>"
  },
  "policy": {
    "ref": "<policy-source>",
    "config_digest": "sha256:<effective-config>",
    "rules_digest": "sha256:<rules-and-versions>",
    "task_contract_digest": null
  },
  "plan": {
    "required_checks": ["ai-code-traceability"],
    "pending_delivery_checks": []
  },
  "gate": {
    "complete": true,
    "decision": "fail",
    "blockers": ["diag-001"]
  },
  "checks": [
    {
      "id": "ai-code-traceability",
      "rule_version": 1,
      "required": true,
      "severity": "error",
      "applicability": "applicable",
      "execution": {"status": "completed"},
      "verdict": "fail",
      "matched_entities": 1,
      "diagnostics": [
        {
          "id": "diag-001",
          "fingerprint": "<rule-symbol-violation-fingerprint>",
          "file": "src/test/java/OrderTest.java",
          "range": {"start_line": 42, "end_line": 49},
          "message": "新增测试缺少团队要求的来源声明",
          "evidence": {"symbol": "OrderTest#should_create_when_valid", "annotations": []},
          "fix": "按实际来源补齐约定字段，并声明配套依赖后复验",
          "recheck": {
            "argv": ["qualitygate", "check", "--worktree", "--base", "<base-commit>", "--profile", "full", "--format", "json"]
          }
        }
      ]
    }
  ],
  "summary": {"checks_total": 1, "pass": 0, "fail": 1, "skipped": 0, "incomplete": 0, "diagnostics_total": 1}
}
```

### 8.2 执行证据与复验

- 快照覆盖检查所需的源码、配置、测试和构建输入，标明提交、暂存区或工作区来源。环境记录包含影响结果的工具版本、依赖锁定信息与必要配置；不采集凭据。
- 命令检查记录实际 `argv`、工作目录、开始 / 结束时间、退出码、执行状态以及日志 / 报告路径与摘要。内部规则记录使用的适配器版本、事实来源和匹配数量。
- 诊断标识与稳定指纹支持跨次运行匹配；指纹优先使用规则、符号和违规类型，避免仅因行号变化就成为“新问题”。执行证据仍须绑定每次实际快照。
- `recheck` 保留原检查范围、基准、profile 与任务契约等必要上下文。局部复验可以指导修复，最终交付仍需完整验证。
- 缓存仅在相关输入、规则、工具和环境条件可校验一致时复用；无法证明一致时重新执行。M1 默认重新执行，后续再引入缓存。
- CI 重新执行或校验可信执行服务提供的证据；本地 JSON 的 `pass` 字段或摘要本身不构成可信验收凭证。

报告同时支持 table 和 Markdown（MR 检视用），与 JSON 使用同一门禁计算结果，展示跳过原因、未完成项、过滤统计及修复建议。工具生成报告；向外部 MR 发布由接入方按现有授权流程执行。

## 9. 验收标准

验收采用证伪式原则：**仿真环境跑不通可以证伪，仿真环境跑通不能证明真实环境一定正确。**
内置 `fixtures/minimal`、`typical`、`stress` 与独立 `golden` 构成回归全谱系；
每条规则必须在合规样例上 pass、违规样例上 fail。`qualitygate selfcheck` 执行全量，
`--fixture minimal` 用作快速 CI 门禁，`--rule <rule-id>` 用于定位单条规则。
人为放宽规则（例如允许空提交信息）必须使独立 golden 比对失败。
每次工具改进后，全谱系结果须与 golden 一致；失败必须指出 fixture、断言和输入。
所有检查报告明确列出“已验证形态”“已知边界”“未验证假设”，无问题结论限定为
“在已验证形态下未发现问题”；违规与未完成不得使用该结论。真机结果作为已知边界
和试点收益证据记录，不以“在真实仓库验证通过”代替可重复的 fixture 验收。

### 9.1 功能验收

1. **适用性与错误处理**：受支持仓库可完成 `init` + `check`；未知生态、错误配置和缺失工具不崩溃，输出能力缺口与明确状态。空计划或全部跳过不能报告已验证通过。
2. **门禁完整性**：覆盖违规、必需工具缺失、解析失败、超时、前置失败及报告缺失场景；分别验证判定维度和 0 / 1 / 2 退出码，显示过滤不改变门禁。
3. **闭环有效**：在可控 fixture 仓库中，agent 收到诊断后修复并复验，结果与 golden 一致；必需测试未执行或断言失败时不能满足交付条件，零测试执行不能算测试通过。真实试点中的修复结果另列为已知边界和收益证据。
4. **任务验收覆盖**：规范通过但任务测试失败时，任务门禁仍阻塞；未提供任务契约时不宣称任务完成；快速 / 局部检查明确显示剩余交付项。
5. **增量准确性**：用只改一行但存在历史告警、符号移动 / 重命名、变更影响未修改文件、基准缺失等样例验证各模式；检查范围与阻塞依据可解释。
6. **快照一致性**：暂存区与工作区内容不同、执行期间文件被修改、报告属于旧提交或旧配置时，不得错误复用结果。
7. **规则与策略维护**：来源章节变化可追踪；删除标记及其依赖、禁用强制规则、降低级别、删除任务验收项或更改验证脚本，不能未经策略流程就使原有门禁通过。
8. **来源声明边界**：无来源声明规范的仓库不误报；配置为仅检查 AI 参与变更但范围证据缺失时，必需检查未完成，不从 author 或标记反推来源。
9. **结果可复验**：JSON 可解析、带协议版本和稳定诊断标识；记录实际执行命令与证据，table / Markdown / JSON 的门禁结论一致。
10. **生态扩展**：第二种语言验证文件与项目路由、语法与语义适配边界；自定义规则仅在已声明能力与稳定协议内复用，不要求复杂语义纯 YAML 实现。

以上为目标版本验收项，各阶段按 §10 承诺范围验收；后续能力不得在前期被标记为已支持。

### 9.2 试点收益验收

试点开始前选定真实仓库、历史问题样本和可比较的任务类型，记录既有工具的基线。在接入前后或可比任务组中观察以下指标，避免将已有检查的收益重复归因于 qualitygate：

| 指标 | 口径 | 用途 |
|---|---|---|
| 真实违规拦截 | 在提交 / MR 前发现且人工确认的独立违规数，区分已有工具结果与新增规则 | 判断是否覆盖了真实问题 |
| 误报率 | 人工判定为误报的诊断数 / 已复核诊断数，同时记录未复核数量 | 决定规则能否升级为强制门禁 |
| agent 修复成功率 | 收到有效诊断后在约定预算内完成修复并复验通过的案例数 / 可自动修复的有效案例数 | 判断反馈是否可行动 |
| 人工检视返工 | 可比任务中与已覆盖规则相关的检视意见数、返工轮次及人工耗时 | 判断是否减少验收负担 |
| 检查成本与覆盖 | quick / full 的耗时分布、必需检查完成率、能力缺失与跳过原因 | 防止以漏检换取速度 |

试点启动时由团队确定样本量、观察周期和接受阈值，并纳入验收记录；文档不预设未经测量的收益数字。只有在准确性、修复反馈和检查成本达到试点标准后，再扩大规则与语言范围。

## 10. 里程碑

| 阶段 | 内容 | 完成依据 |
|---|---|---|
| M1：单仓库闭环 | 内置 fixture 全谱系与 selfcheck；代码快照、diff、有限内置规则与配置、命令式任务契约、一个现有验证命令、JSON / 门禁状态、受信策略差异检测与 CI 接入；实现必要的单生态解析能力 | 全谱系与 golden 一致，规则正反例和变异证伪生效；检查 → 修复 → 复验；验证必需检查缺失和策略变化不能静默放行；真机试点基线另列为已知边界 |
| M2：验证工具聚合 | 扩展首个生态（优先 Java）的编译 / 测试 / 静态 / 覆盖率报告适配；落实增量诊断与执行证据；按历史问题增加依赖边界、接口兼容性等高价值检查 | 工具结果绑定正确快照，误报和修复成功率达到试点预定标准 |
| M3：第二种语言验证 | 按试点需求选择 Python / TypeScript / Go 中一种，验证文件与项目适配接口及混合仓库；按需求接入人工验收和外部来源记录 | 在第二个生态复现验收闭环，记录抽象差异和能力缺口后再扩展支持矩阵 |
| M4：稳定扩展接口 | 在前期验证基础上版本化自定义规则 DSL 和适配协议，提供 MR 接入、Markdown 预检报告及规则迁移说明 | 团队可在公开能力范围内添加私有规则，协议与策略升级有兼容性验证 |

M1 优先交付 fixture 全谱系证伪门禁和实际验证命令；真实仓库收益按 §9.2 单独收集并说明边界，不能替代 golden 回归。通用解析框架、语言数量和规则数量不单独作为产品价值的验收依据。

## 11. 设计参考

- [Anthropic：Effective harnesses for long-running agents](https://www.anthropic.com/engineering/effective-harnesses-for-long-running-agents)：任务清单、进度记录与实际测试支持持续工作；本项目聚焦其中的验证反馈与交付门禁。
- [Tree-sitter 官方说明](https://tree-sitter.github.io/tree-sitter/)：提供语法树生成与增量解析能力；项目语义分析需由本项目的生态适配层另行提供。


## 11. 规则契约与测试有效性增量

- 有限 DSL 支持必需文本 `require_pattern`；`min_count` 适用于所有合法实体/变更组合，只计触发实体。空扫描违规与能力/解析不完整必须分离。
- 命令及任务可显式配置 `test_effectiveness`，对每个新增或内容修改的独立测试文件要求同一用例新代码通过、旧代码出现结构化断言失败。
- 基线采用本次比较的已解析 base；组合快照只移植声明的测试/辅助文件，保留删除和模式，拒绝生产路径重叠及受保护策略/构建输入替换。
- 两次执行共用截止时间并保留各自输入、工具、日志和逐用例证据。缺失报告、未知失败类型、编译错误、跳过、超时及快照不一致不能成为有效反例。
- 不自动推断规则、测试命令或豁免；首版不抽取内嵌测试。沿用原有架构、规则驱动、策略审查与 0/1/2 结果语义。

配置和需求到测试映射见 [测试有效性](docs/test-effectiveness.md) 与 [规则 DSL](docs/custom-rules.md)。

## 12. 跨 Agent 验收能力演进

依据 2026-09-15 的模型能力与 Agent 工程研究，后续演进聚焦跨模型、跨 Agent 的任务验收。
Agent 负责生成规则、测试与修复提案；CLI 保持无需模型调用的验收核心，沿用快照、受信策略、
任务契约及 0/1/2 门禁语义。模型版本和公开评测不成为兼容性要求或真实收益证据。

完整需求、研究来源、适用边界、阶段与需求到测试映射见
[跨 Agent 验收能力演进需求 SPEC](codespec/requirements/agent-acceptance-evolution.md)。

| 编号 | 演进方向 | 目标验收 |
|---|---|---|
| EVO-01 | 按任务类型组织验收 | 提供有明确前提的任务模板；缺陷修复可验证旧代码反例，合法重构不默认要求旧版本失败 |
| EVO-02 | 面向 Agent 的修复反馈 | 有界精简反馈保留诊断、证据、复验参数与剩余交付项，和完整报告的门禁一致 |
| EVO-03 | 跨 Agent 验收闭环 | 至少两种 Agent 使用相同 CLI 契约完成修复与复验，最终完整报告绑定实际组合后的代码快照 |
| EVO-04 | 从失败案例演进规则 | 区分生成、人工确认与独立验证案例，复用候选验证、独立批准、推广和回退机制 |
| EVO-05 | 真实收益与交付成本评估 | 保留既有工具基线、独立收益归因、全部尝试与人工复核，缺失数据和零分母保持未知 |

上述需求分阶段实施，工程交付与真实收益分别验收。优先建立真实试点、任务模板和修复反馈，
再按独立证据扩展规则、生态与结果交换。试点项目、Agent、样本量、周期、预算和数字阈值必须在
观察结果前确定，按 §9.2 和 [试点协议](docs/pilot.md)独立验收；不改变现有 M1–M4 的完成状态。

阶段 A 按用户确认的本地限制，以同一 Codex 的中、低能力模型配置组模拟两种 Agent，
固定其余运行条件；这不替代后续跨产品验证。已提供版本化 Rust 任务模板、受控基线回归与
显式启用的模型连接探测，细化需求及当前证据见[阶段 A 记录](docs/pilot-phase-a.md)。

阶段 B 已实现 `check --feedback`、可复用报告上下文和外层 Rust 修复循环，补充组合分支回归
及 Codex 双模型实际源文件提案与完整复验。实现、质量门禁与试点边界见
[阶段 B 记录](docs/pilot-phase-b.md)；8 个任务 / 7 天的收益验收仍独立执行。

阶段 C 已实现案例来源谱系、受保护 suite v2、只读 `pilot summarize`、失败不丢失的
试点分母和其余 Rust 任务模板，并离线复查阶段 B 的全部成功与失败调用。实现与证据见
[阶段 C 记录](docs/pilot-phase-c.md)；真实独立案例、七天观察和授权推广仍单独验收。

阶段 D 已实现观察前 `pilot seal`：完整治理字段、real 任务真值、Codex 中/低模型与
existing-tools/qualitygate 的平衡 cohort 矩阵必须在无 observation 时封存。后续可补充实际
模型身份和观察；其它计划变化使汇总保持未完成。摘要不认证身份或时间，具体 8 个生产任务、
复核人、金额上限和持久归档仍需团队在真实七天试点启动前填写。设计与证据见
[阶段 D 记录](docs/pilot-phase-d.md)。

阶段 E 已实现 `pilot authorization-subject` 和汇总时的外部 DSSE/Ed25519 owner 授权复验。
签名绑定 repository、试点、plan seal、owner/reviewer、样本矩阵和七天窗口；trust store 与记录
必须位于被测仓库外，并检查 key scope、有效期、最大年龄及撤销。缺失授权保持描述性输出和
`protocol_ready=false`。签名不提供独立可信时间，也不替代七天观察和 reviewer 最终接受；
设计与证据见[阶段 E 记录](docs/pilot-phase-e.md)。

阶段 F 已实现 `pilot acceptance-subject` 和独立 reviewer 最终签名。纯领域逻辑聚合所有
Qualitygate 组与匹配对照的 9 项必需阈值，manifest、report 引用、启动授权或指标变化都会使
旧验收主题失效。只有全部检查为 met 才能签 accepted；认证 rejected 返回阻塞状态。该能力不补造
真实七天数据，也不从非结构化金额上限推导结论；设计与证据见[阶段 F 记录](docs/pilot-phase-f.md)。

阶段 G 为新试点提供 manifest schema v2 结构化预算：封存币种、价格时间、定价来源、
人工时薪和 32 个运行单元共享的总上限；纯领域计算累加全部尝试及人工审查时间，并将第十项
总预算检查加入独立 reviewer 主题。缺费用保持未知，已知超限拒绝接受；旧 v1 封存记录保持
原九项语义。设计与证据见[阶段 G 记录](docs/pilot-phase-g.md)。

阶段 H 为新试点提供 manifest schema v3 独立任务分层：预先固定缺陷修复/重构各 4 个，
按不同输入核对类型配额，并拒绝跨输入复用任务 ID 或契约摘要。汇总输出可复核任务清单；
v1/v2 已封存计划保持原摘要和验收语义，v3 沿用 v2 十项阈值。真实任务独立性仍需外部
来源和人工复核。设计与证据见[阶段 H 记录](docs/pilot-phase-h.md)。

阶段 I 为新试点提供 manifest schema v4 来源工件绑定：每个独立任务预先声明 issue/commit
来源 ID、受限相对文件、长度、摘要和选入时间；封存及后续每次验收输出都复核归档字节。
缺文件、越界、符号链接或字节漂移保持未完成。旧 v1/v2/v3 记录维持原摘要和阈值语义；
摘要不能证明来源内容与时间真实。设计与证据见[阶段 I 记录](docs/pilot-phase-i.md)。

阶段 J 为新试点提供 manifest schema v5 交替顺序契约：封存完整运行单元排列，
按任务类型和请求模型平衡两种流程的先后；观察记录声明实际启动位置，缺失或偏离
阻止最终接受。序号真实性须对照外部启动日志。旧 v1–v4 封存摘要保持不变。
设计与证据见[阶段 J 记录](docs/pilot-phase-j.md)。

阶段 K 为新试点提供 manifest schema v6 初始报告与修复停止审计：每运行单元在封存前
绑定一份完整初始报告，封存连续无进展上限；汇总从完整复验报告重算错误债务真子集、
尝试次数与累计耗时。超预算或连续两次无进展后继续执行为协议偏离；旧 v1–v5
摘要和阈值语义保持不变。真实耗时与 Agent 日志仍须外部复核。设计与证据见
[阶段 K 记录](docs/pilot-phase-k.md)。

阶段 L 为新试点提供 manifest schema v7 逐运行模型采集：观察记录绑定外部归档 JSON 的
路径、摘要、长度、采集时间、请求模型、Agent/harness 身份，以及报告的实际模型或明确
未知原因。缺记录和不匹配保持未完成；明确未知不再仅因此阻断请求配置层面的试点协议。
归档摘要不能认证服务商路由与本地采集时钟，须由 reviewer 对照外部日志。旧 v1–v6
封存与验收语义保持不变。设计与证据见[阶段 L 记录](docs/pilot-phase-l.md)。

阶段 M 为新试点提供 manifest schema v8 逐尝试模型采集，包括失败、超时和
放弃的尝试；缺失保持未完成，已报告的单元内路由漂移阻止最终接受。
旧 v1–v7 语义保持不变，设计与证据见[阶段 M 记录](docs/pilot-phase-m.md)。

阶段 N 为新试点提供 manifest schema v9 逐尝试执行记录，核对开始/结束时间、
耗时、状态、快照、报告及封存的 harness 身份，并审计归档启动时间与声明
顺序。缺失保持未完成，时间逆序为协议偏离；旧 v1–v8 语义保持不变。
设计与证据见[阶段 N 记录](docs/pilot-phase-n.md)。

阶段 O 为新试点提供 manifest schema v10 非财务验收：不要求人工时薪、
计价来源/时间、币种或金额上限，仍封存 8 项任务/7 天、执行资源预算、
任务来源和逐尝试证据。最终主题要求八项非财务阈值已知且达标；
金额缺失保持未知，不阻断该验收。旧 v1–v9 封存与验收语义保持不变。
设计与证据见[阶段 O 记录](docs/pilot-phase-o.md)。A–O 工程契约已实现，
后续是[真实试点准备度与独立验收](docs/pilot-readiness.md)，不能由本地
fixture、同一 Codex 的两模型或摘要复查替代。

## 13. Skill 发布与文档站点

| 编号 | 发布要求 | 验收证据 |
|---|---|---|
| DIST-01 | Skill 与 CLI、Bazel 的版本一致；版本标签触发 GitHub Actions 先验证 Rust 质量门禁、再构建并核查各平台资产、发布可下载归档及 SHA-256；无标签的手动试运行不发布 | `tests/quality/skill_package.rs`、`tests/bazel.rs`、[发布工作流](.github/workflows/release.yml)的实际运行记录和 Release 资产 |
| DIST-02 | `main` 的静态 Pages 站点提供当前版本的下载、安装和文档入口；站点内链接及版本在仓库质量门禁中校验，部署结果由 GitHub Pages 实际地址复核 | `tests/quality/site.rs`、[Pages 工作流](.github/workflows/pages.yml)的实际运行记录和在线页面 |

发布工作流成功与正式试点收益验收相互独立；站点只陈述已实现能力，
不能把受控 fixture 或未启动的 8 任务/7 天试点称为真实收益。

## 14. 类型化决策与可选判断

| 编号 | 要求 | 验收证据 |
|---|---|---|
| DEC-01 | 可导出版本化 decision、feedback 和 project-rule Schema；`check`、feedback、rule validation、selfcheck 及策略/试点摘要可选择带判别类型的 envelope，保留原始 payload、证据摘要、warning、gap、pending 与 omission；未完成不得表示为通过 | `tests/decision_envelope.rs`、[协议](docs/decision-protocol.md)、[证据矩阵](docs/acceptance-evidence.md) |
| DEC-02 | 外置 provider 仅用固定命令、版本与输入摘要对 warning 做 shadow/advisory 评估；deterministic、probabilistic、abstained、execution-gap 类型闭合，非法输出或失效校准不改变原门禁并留下原始证据 | `tests/judgment_provider.rs`、`domain::judgment::tests`、[协议](docs/decision-protocol.md) |
| DEC-03 | warning 试点隔离校准与验证时间，使用独立标签计算校准与风险指标，对高优先级建议抽样审计；缺证据保持未完成，策略变更仍走外部批准流程 | `tests/judgment_provider.rs`、[协议](docs/decision-protocol.md)、[策略演进](docs/policy-evolution.md) |
