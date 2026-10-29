# 内置与自定义规则

[English](../../en/02-reference/02-rules.md) · [本卷目录](README.md)

## 规则来源与类别

规则来自版本匹配的 Skill 资产、`qualitygate/rules` 下的仓库规则包，或配置的旧版自定义规则目录。
类别只组织发现；规则包和具体规则仍需显式选择。配置分别保存来源身份、参数、严重级别、语言、
所需能力和审核元数据。

内置目录包含语言无关的提交与文件规则，以及按需启用的 Java、Python、TypeScript/JavaScript、Go、
Rust、Shell、C 和 C++ 包。适用时结构规则使用 tree-sitter。许多安全、日志、风格和 API 检查是逐行
模式审查信号；命中并不能证明漏洞可利用、数据流、运行时行为或标准符合性。

`import-boundary` 在仓库提供经审核的 `forbidden_imports` 前保持禁用；`security-sensitive-api` 和
`todo-marker` 默认是 warning。启用语言规则前应阅读其精确定义和能力限制。

## 项目规则协议

版本 1 YAML 规则包声明身份、适用范围、来源审核、实体、断言、消息与资源边界。断言可要求或禁止
有界文本，也可要求匹配实体集合非空，但不能执行任意代码或静默扩张能力。

下表说明 v1 契约字段。抽取、验证、生成与独立采用步骤统一见
[编写教程](../01-user-guide/06-schema-guided-repository-rules.md)。

| 契约字段 | 含义 |
| --- | --- |
| `schema_version`、`id`、`version` | 协议版本与稳定规则身份。 |
| `source` | 仓库相对文档、精确章节及 `rules source` 返回的 digest。 |
| `language`、`applies_to` | 显式语言和路径/provenance scope。 |
| `requires_capabilities` | evaluator 必须具备的事实；缺失时结果为 incomplete。 |
| `when` | commit、file、test、comment 或 import 的封闭 subject/change selector。 |
| `then` | 支持的名称、文本、marker、dependency、count、required-path、line 或 word assertion。 |
| `binding` | 需要追溯时使用的 annotation、comment 或 Git trailer 类型化关联。 |
| `fix` | 从同一来源条款推导的可执行修复说明。 |

### 结构化 Schema 契约

project-rule JSON Schema 是 CLI、Skill、生成、校验及不可变策略加载共享的可执行序列化契约。对象通过
`additionalProperties: false` 闭合；条件分支把每种 entity 与必需 capability 关联，并限制完整文件
清单、marker binding 和 dependency evidence 等字段组合。

`rules schema` 导出运行时副本，匹配版本的 Skill 在
`references/schemas/project-rule.schema.json` 携带同一份 Schema。两份 JSON 解析后相等只证明协议兼容，
不代表来源获批。CLI 还会执行 Rust regex/glob、受限路径、唯一身份、精确来源章节 hash、capability
组合和有限预算等语义校验。

## 选择规则协议

Qualitygate 1.0.0 保留 v1/v2 兼容性，并正式发布 V3 命名集合与词法 lambda。
CLI 版本和规则的 `schema_version` 是独立概念。已发布的 0.5.9 二进制没有声明
`rules.operators.v3`；使用 V3 时安装 1.0.0，并在编写前查询实际运行时能力。

| 规则格式 | 由匹配 CLI 导出 | 使用场景 |
| --- | --- | --- |
| v1 | `rules schema --format json` | 固定的文件、文本、命名、标记与依赖断言。 |
| v2 | `schema operator-rule` | 对选中实体集合求值的类型化有界表达式。 |
| v3 | `schema operator-rule-v3` | 命名事实集合、词法 lambda 变量与标量转换。 |
| 混合包 | `schema project-rule-family` | 校验同时包含 v1、v2、v3 的规则包。 |

```bash
qualitygate --version
qualitygate capabilities --format json
qualitygate schema operator-rule-v3
qualitygate schema project-rule-family
```

实际能力、导出的 Schema 和来源绑定必须匹配所选协议。仍适用的 v1/v2 规则可以保留原格式；
只有新要求需要 V3 能力时才迁移，CLI 升级本身不要求改写规则。

## V2：有界表达式

能力 `rules.operators.v2` 引入带必需类型化 `expression` 的 project-rule `schema_version: 2`。
现有 v1 Schema 与语义保持不变。通过 `schema operator-rule` 导出新契约；默认 `rules schema`
仍描述 v1。[打包算子参考](../../../skills/qualitygate-cli/references/operators.md)提供完整操作集、示例、预算及编写流程。

### 表达式语义

表达式通过布尔组合、`all/any/filter/count`、类型化比较、成员判断及正则，组合已有文本/AST 实体事实。
字段只能是谓词当前实体的 `name`、`text`、`path`；不运行任意代码或动态字段。
原有固定断言继续叠加生效，包括声明保留和依赖检查。表达式集合仅包含规则选中的触发实体，
不增加 adapter 本身不具备的语义知识。

类型/形状校验检查全部分支。事实采集必须先完整完成，声明的依赖/来源事实需保持绑定。
完整空集合有明确语义：all 为 true、any 为 false、count 为零；要求至少一个实体时显式增加计数。
解析失败、事实缺失和预算耗尽保持未完成。即使算子未完成，之前固定断言发现的违规仍保留。

### 求值预算与报告

每次求值的嵌套操作共享预算：256 个节点、深度 16、50,000 个实体、32 MiB 事实、100,000 次操作、
32 MiB 字符串扫描及 30 秒截止时间。字面量与正则编译还有独立严格上限。
布尔短路不能隐藏声明来源的不完整事实或绕过类型校验。原有采集预算仍生效，不引入通用代码执行环境。

主集合与所有命名来源共享同一采集截止时间、实体数量和字节额度。选择器在复制规则事实前
预留额度；语法解析与实体匹配继承剩余时间。预算耗尽时检查保持不完整，并保留已经确认的
固定断言违规。

报告保留原有快照、策略和来源审查绑定。完成求值时输出操作/扫描统计；条件为假产生集合级违规。
混合编写包标识可导出的 Schema 家族及每个规则文件的实际契约。
Schema 合法、本地来源审查、独立策略批准及解释是否充分仍是不同的保证。


### 编写并验证表达式

先确认 `capabilities` 包含 `rules.operators.v2`，再导出 `schema operator-rule` 获取完整 v2 形状。
以下片段要求至少选中一个实体，且每个名称都符合模式。需要配合 `when.entity: test_method`、
`test_methods` 能力、身份、来源绑定及其他必需字段：

```yaml
expression:
  op: and
  args:
    - op: compare
      left: {op: count, input: {op: entities}}
      relation: ge
      right: {op: number, value: 1}
    - op: all
      input: {op: entities}
      predicate:
        op: matches
        value: {op: field, name: name}
        pattern: '^test_'
```

将完整规则保存为 `candidate.yaml`，执行 `rules validate candidate.yaml`，再在获准编写流程中运行
`rules generate --input candidate.yaml`。V1 拒绝 `expression`；生成不会启用规则。

## V3：命名来源与词法 lambda

`rules.operators.v3` 提供独立的 `schema_version: 3` 契约，通过
`schema operator-rule-v3` 导出。V1/V2 语义及 `schema operator-rule` 保持不变；
family Schema 包含三个版本。使用匹配的预编译 qualitygate 即可，不需要安装 Rust、Node.js 或 Python。
使用前通过 `qualitygate capabilities --format json` 确认包含 `rules.operators.v3`；
版本号相同本身不能证明支持 V3。

Python 与 Node.js 项目使用相同的 YAML/JSON 表达式，通过 `applies_to.paths`、来源选择器
和字符串操作配置文件扩展名及对应约定。仓库快照仍需要 Git。Lambda 函数体是声明式表达式，
不执行 JavaScript/Python 回调；配置的命令检查仍需要对应工具与运行时。
Unix 集成测试在 `PATH` 仅保留 Git 时验证规则校验和 full 执行，覆盖缺失对应文件与规则来源过期。

规则可以声明多个独立选择的事实集合、绑定变量、引用外层实体，并组合标量 `map`、`distinct`、
`contains`、`subset`、`concat` 和字面量 `replace`。全部声明来源必须完整且绑定同一快照；
缺失事实不能当成空集合。顶层触发与固定断言语义不变。命名文件来源可显式包含未修改的上下文，
但不扩大已采集快照或绕过排除策略；AST 全量选择仍不支持。

[完整 V3 参考与源文件／测试对应示例](../../../skills/qualitygate-cli/references/operators-v3.md)
定义类型、来源、变量作用域、物化预算和诊断。文件对应检查只证明文件关系；行为证据仍由配置的测试
及适用时的 `test_effectiveness` 提供。qualitygate 的职责仍是规则化与规则执行。

V3 的 `distinct`、`subset` 使用有界标量索引减少重复的逐对比较。不同值较少时保持线性处理，
一般集合使用稳定排序和精确比较；索引存储、比较、字符串扫描与索引移动都消耗既有共享预算。
`distinct` 保留首次出现顺序及原始来源。少量子集查询直接扫描，避免为大右集合额外排序。
集合操作数仍完整求值；空左集合不构造无用索引。
更大的输入仍可能耗尽预算，此时结果保持未完成。

## 报告、文件契约与规则评估

分析器棘轮属于 command check，会在基线与当前两个不可变快照上归一化报告。file contract 可覆盖
声明的完整文件清单，包括配置要求的未改动 owner/test 文件。二者都不应伪装成新内置规则。

规则评估区分可由有界文本/结构证据表达的提案，与依赖类型、数据流、控制流、生命周期、依赖解析或
人工判断的提案。不支持的语义规则应保留为能力缺口，不能用误导性的正则近似。规范来源审核应把
外部文档的准确章节映射到可执行行为，并保存来源摘要、审核人、日期和已知差异。

实现分派、lint 复用、diff/实体匹配及各算法的技术原理见
[规则引擎实现](../03-architecture/06-rule-engine-implementation.md)。

---

[上一章：CLI 命令](01-cli-commands.md) · [全书目录](../README.md) · [下一章：项目与工具报告](03-projects-and-tool-reports.md)
