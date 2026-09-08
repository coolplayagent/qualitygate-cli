# qualitygate-cli 需求文档

> 版本：v0.1（草案）
> 状态：待评审
> 关联讨论：仓库规范检查器——把 AGENTS.md / 团队规范从"提示词记忆"变成"可执行检查器"

## 1. 项目定位

qualitygate-cli 是一个**仓库规范检查器**：将散落在团队文档（AGENTS.md、开发规范、提交规范等）中的约定，转化为可执行、可判定、可进流水线的检查命令。AI 用它自证合规，人用它验收 AI，团队用它落地规范。

**核心价值**：规范从"人记得、AI 猜"变成"命令判定、机器门禁"。

## 2. 设计原则

| 原则 | 说明 |
|---|---|
| **增量视角** | 只检查本次改动引入的问题（diff-mode），不报存量噪声 |
| **规则声明式** | 规则用 YAML 声明，启用/禁用/严重级别可配置 |
| **分级输出** | error（必须修）/ warning（建议修）/ info（提示），附修复建议 |
| **可进流水线** | exit code 语义化（0=通过，1=有 error），可直接作为 MR/CI 门禁 |
| **零假设** | 引擎不内建"仓库必须有 X"；所有生态依赖走先决条件机制，不满足则明确 skipped |
| **跨语言通用** | 引擎与语言解耦，同一二进制可检查任意语言/生态仓库 |

## 3. 功能需求

### 3.1 变更规范检查（diff 级，核心）

| 检查项 | 检测逻辑 | 说明 |
|---|---|---|
| AI 生成代码可追溯 | 新增测试方法是否带团队约定的 AI 生成标记（绑定见 §6.4） | 标记方式按仓库配置 |
| 配套依赖存在性 | 使用了标记但依赖声明缺失 → error | 如 Java 生态的 aitool 依赖 |
| 测试命名规范 | 新增测试方法是否匹配 `should_[ExpectedBehavior]_when_[Condition]` | 按测试框架适配 |
| 参数化测试检测 | 同一测试类新增 3+ 个相似 Given-When-Then 独立方法 → warning | 建议重构参数化 |
| 注释语言 | 新增注释是否团队约定语言（可配置：中文/英文/双语） | |
| 换行符保持 | 修改文件行尾与原文一致（LF/CRLF），新文件默认 LF | |
| 提交信息格式 | 匹配 `[ID]type: description` 风格（ID/type 集合可配置） | |
| 新依赖声明检查 | 新增 import 的第三方符号未在依赖清单声明 → error | 编译前拦截 |

### 3.2 构建 / 静态质量聚合

| 检查项 | 检测逻辑 | 说明 |
|---|---|---|
| 构建命令规范 | 检查 CI 配置/脚本中的构建命令是否带必需参数（如 Maven `-s settings.xml`） | 按生态配置 |
| 静态检查增量过滤 | 拉取静态检查结果（checkstyle/spotbugs/pmd/CodeCheck…），只报本次改动文件的问题 | 存量不阻塞新变更 |
| 覆盖率增量 | 对接增量覆盖率工具：本次改动引入的未覆盖分支 | 支撑"新增代码 100% 覆盖"验收 |

### 3.3 流程门禁

| 功能 | 说明 |
|---|---|
| 提交前验证清单 | 一键顺序执行：依赖验证 → 编译 → 测试 → 构建 → 静态检查，聚合结果 |
| MR 预检报告 | 对 MR 输出规范检查报告，作为检视前置/检视意见素材 |

## 4. 通用化架构：三层解耦

```
┌─────────────────────────────────────────────────┐
│  L0 核心引擎（语言无关）                          │
│  diff 解析 / 文件分类 / 规则引擎 / 报告 / CI 门禁  │
├─────────────────────────────────────────────────┤
│  L1 语言适配器（Lang Adapter）                    │
│  把源码解析成"统一结构视图"，每种语言一个          │
│  基于 tree-sitter（一个库支持 150+ 语言）         │
├─────────────────────────────────────────────────┤
│  L2 规则层（声明式 + 生态包）                     │
│  规则 = 针对结构视图的谓词，YAML 声明             │
│  core（通用）/ lang-java / lang-python / ...     │
└─────────────────────────────────────────────────┘
```

**要点**：规则引擎永远不直接碰源码，只碰"结构视图"。换语言 = 换适配器 + 换规则包，引擎零改动。

## 5. 多语言支持

### 5.1 统一结构视图（Canonical Structure View）

不同语言要检查的语义概念相同，只是语法不同：

| 概念 | Java | Python | TypeScript | Go | Shell |
|---|---|---|---|---|---|
| 测试方法 | `@Test void x()` | `def test_x()` | `it()/test()` | `TestXxx` | — |
| 注解/标记 | `@AIGenerated` | `@pytest.mark...` | `@Decorator` | 注释 | 注释 |
| 依赖声明 | pom.xml | pyproject/requirements | package.json | go.mod | — |
| 导入引用 | `import` | `import` | `import` | `import` | `source` |
| 注释 | `//` `/* */` | `#` | `//` `/* */` | `//` `/* */` | `#` |

适配器输出统一 JSON：

```json
{
  "language": "java",
  "tests": [{"name": "should_x_when_y", "line": 42,
             "annotations": ["AIGenerated"], "is_added": true}],
  "imports": [...],
  "dependencies": {"declared": [...], "used_undeclared": [...]},
  "comments": [{"text": "...", "line": 10, "is_added": true}]
}
```

### 5.2 语言适配器接口

```rust
trait LangAdapter {
    fn detect(root: &Path) -> bool;        // 识别语言/生态
    fn parse(&self, file: &Path) -> FileStructure;  // 源码 → 结构视图
}
```

解析统一用 **tree-sitter**（CST → 结构视图只需对每种语言写 query，不写 parser）。

### 5.3 多语言仓库

混合语言仓库（如 Java + Shell + Python）按**文件语言路由**：diff 中每个文件 → 语言检测 → 对应适配器解析 → 该语言规则集判定。同一 diff 内不同语言各跑各的规则。

## 6. 规则系统设计

### 6.1 规则分层

| 层 | 定义 | 例子 | 复用性 |
|---|---|---|---|
| **L0 通用规则** | 与语言完全无关 | 提交信息格式、换行符保持、diff 规模告警 | 所有仓库直接用 |
| **L1 语言映射规则** | 判定逻辑通用，只依赖结构视图 | 测试命名、AI 标记完整性、依赖已声明 | 换语言只换适配器 |
| **L2 生态规则** | 依赖特定生态事实 | Java：aitool 依赖配套；Python：pytest marker | 按生态打包 |

生态包文件布局：

```
qualitygate/rules/
  core/          commit-message.yaml, line-ending.yaml, comment-language.yaml
  lang-java/     ai-code-traceability.yaml, junit-naming.yaml
  lang-python/   pytest-naming.yaml, dep-declared.yaml
```

### 6.2 规则 DSL（声明式）

```yaml
# lang-java/ai-code-traceability.yaml —— 示例
id: ai-code-traceability
language: [java]
severity: error
prerequisites:                        # 先决条件——不满足则 skipped
  - kind: dependency_present
    file: pom.xml
    group: com.huawei.dfv.s3.common
    artifact: aitool
binding:                              # 标记方式绑定（init 探测 + 团队配置）
  marker:
    type: annotation                  # annotation | comment | git_trailer | none
    name: AIGenerated
    fields: [author, date, description]
when:                                 # 结构视图上的触发条件
  entity: test_method
  added: true
then:
  require_annotation: AIGenerated
fix: "添加 @AIGenerated(author=..., date=..., description=...) 并确认依赖已声明"
```

### 6.3 先决条件与三态语义

**核心机制：先决条件不满足 → 规则进入 `skipped`，而不是 pass/fail。**

| 状态 | 语义 |
|---|---|
| `pass` | 仓库支持且符合 |
| `fail` | 仓库支持但违反 |
| `skipped` | 仓库不具备先决条件——报告明确提示"为什么跳过 + 如何启用" |

这保证：**任何规则在任意仓库都有明确、自洽的状态，且报告解释原因**。没有注解包的仓库不会报错，也不会假装通过。

### 6.4 AI 生成代码可追溯性（通用方案）

"AI 生成代码可追溯"是通用需求，但标记方式因仓库而异。先决条件不满足时提供替代绑定：

| 方案 | 适用场景 | 检测方式 | 成本 |
|---|---|---|---|
| 代码内注解 | 有注解包/生态 | 结构视图 annotation 检查 | 低 |
| 注释标记（如 `// @ai-generated`） | 任何语言，无包 | 结构视图 comment 检查 | 低 |
| **git trailer**（`AI-generated: true`） | **完全语言无关** | 读 commit message（引擎通用能力） | 零代码改动 |
| git blame/author 规则 | 按 author/分支识别 AI 提交 | git 元数据 | 零代码改动 |
| 无标记规范 | 团队不要求 | 规则 skipped | — |

结构视图不只解析源码，还融合 git 元数据（author/trailer/blame），使"AI 可追溯"在 git 层就有通用实现。

### 6.5 配置自动发现（init）

```
qualitygate init 扫描仓库:
  ├─ 语言/生态识别（pom.xml / package.json / pyproject.toml / go.mod / *.sh）
  ├─ 生态包自动选择（core + lang-java + ...）
  ├─ 绑定自动探测（是否存在注解包 → 决定标记方式）
  └─ 生成 qualitygate.yaml（团队可改）
```

```yaml
# qualitygate.yaml（init 自动生成）
languages: [java, python, shell]
rulesets:
  - core
  - lang-java
  - lang-python
severity:
  ai-code-traceability: error
  test-naming: warning
custom_rules: ./qualitygate/rules/custom/   # 团队私有规范
```

## 7. 命令面设计

```bash
qualitygate init                              # 自动识别语言/生态，生成配置
qualitygate check --diff <base>..<head>       # 检查 commit 区间改动
qualitygate check --path <file>               # 检查指定文件（工作区）
qualitygate check --mr <url>                  # 检查 MR（内部拉 diff）
qualitygate check --staged                    # 检查暂存区
qualitygate rules list                        # 查看当前生态已启用规则
qualitygate rules enable <rule-id>            # 新规范 = 启一条规则
qualitygate config --show                     # 查看当前配置
# 通用: --format json|table|markdown  --exit-on-error  --severity error
```

## 8. 输出格式

```json
{
  "passed": false,
  "rules": [
    {
      "rule": "ai-code-traceability",
      "status": "fail",                 // pass | fail | skipped
      "severity": "error",
      "language": "java",
      "file": "src/.../XxxTest.java",
      "line": 42,
      "message": "新增测试方法缺少 AI 生成标记",
      "fix": "添加 @AIGenerated(...) 或配置替代绑定（git trailer 等）"
    }
  ],
  "summary": {"pass": 5, "fail": 1, "skipped": 2}
}
```

报告同时支持 markdown 表格（MR 评论/检视用）。

## 9. 验收标准

1. 同一份二进制在任意新仓库 `init` + `check` 不报错、不崩溃；每条规则状态语义明确（pass/fail/skipped + 原因）
2. 报告能指导团队"哪些规范适合本仓库、怎么启用"
3. 增量视角生效：只报本次改动引入的问题
4. exit code 可作为 CI 门禁
5. 多语言仓库按文件语言路由，各自规则正确生效
6. 无 AI 标记规范的仓库：相关规则 skipped 且报告说明，不误报
7. 团队可通过 custom_rules 添加私有规则，不改引擎

## 10. 里程碑

| 阶段 | 内容 |
|---|---|
| M1 | 核心引擎 + tree-sitter + 统一结构视图 + L0 通用规则（提交信息/换行符/注释语言） |
| M2 | lang-java 生态包（AI 可追溯 + 命名 + 依赖声明），首个生产仓库落地 |
| M3 | lang-python / lang-ts / lang-go 生态包（纯 YAML + query，不碰引擎） |
| M4 | custom_rules DSL 开放 + MR 预检报告 + CI 门禁集成 |
