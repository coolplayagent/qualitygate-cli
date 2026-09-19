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

基于 Schema 的流程：

1. 导出匹配版本的 `rules schema`。
2. 记录规范来源及摘要。
3. 使用稳定 ID、明确语言与能力编写候选。
4. 执行 `rules validate candidate.yaml`。
5. 独立审核生成规则及正反例。
6. 发布到项目规则目录，再在策略中显式选择。

未知字段、重复身份、无效正则、能力缺失、语法错误和解析预算耗尽会使验证失败或检查未完成。
规则生成从不等于批准。

### 结构化 Schema 契约

project-rule JSON Schema 是 CLI、Skill、生成、校验及不可变策略加载共享的可执行序列化契约。对象通过
`additionalProperties: false` 闭合；条件分支把每种 entity 与必需 capability 关联，并限制完整文件
清单、marker binding 和 dependency evidence 等字段组合。

`rules schema` 导出运行时副本，匹配版本的 Skill 在
`references/schemas/project-rule.schema.json` 携带同一份 Schema。两份 JSON 解析后相等只证明协议兼容，
不代表来源获批。CLI 还会执行 Rust regex/glob、受限路径、唯一身份、精确来源章节 hash、capability
组合和有限预算等语义校验。

### 从仓库 prose 到可执行规则

`rules source` 从 `AGENTS.md` 或其他仓库策略中抽取唯一无歧义章节，并返回精确 source binding。LLM
只能把明确且可表达的约束翻译进有限 DSL。文件清单、计数、名称、有界文本、marker、import 与受支持
dependency fact 有结构化表示；图、类型、数据流、运行时或人工判断约束必须路由到相应 lint/project
adapter、command check 或 manual decision，不能用正则近似。

[Schema 驱动流程](../01-user-guide/06-schema-guided-repository-rules.md)展示完整的抽取、校验、生成及独立
采用步骤；权威的随包 Agent 流程见 Skill 的[规则编写指南](../../../skills/qualitygate-cli/references/rule-authoring.md)。

## 报告、文件契约与规则评估

分析器棘轮属于 command check，会在基线与当前两个不可变快照上归一化报告。file contract 可覆盖
声明的完整文件清单，包括配置要求的未改动 owner/test 文件。二者都不应伪装成新内置规则。

规则评估区分可由有界文本/结构证据表达的提案，与依赖类型、数据流、控制流、生命周期、依赖解析或
人工判断的提案。不支持的语义规则应保留为能力缺口，不能用误导性的正则近似。规范来源审核应把
外部文档的准确章节映射到可执行行为，并保存来源摘要、审核人、日期和已知差异。

实现分派、lint 复用、diff/实体匹配及各算法的技术原理见
[规则引擎实现](../03-architecture/06-rule-engine-implementation.md)。
