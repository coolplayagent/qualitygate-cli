# 有界算子规则

[English](../../en/02-reference/08-bounded-operators.md) · [分册索引](README.md)

能力 `rules.operators.v2` 引入带必需类型化 `expression` 的 project-rule `schema_version: 2`。
现有 v1 Schema 与语义保持不变。通过 `schema operator-rule` 导出新契约；默认 `rules schema`
仍描述 v1。[打包算子参考](../../../skills/qualitygate-cli/references/operators.md)提供完整操作集、示例、预算及编写流程。

## 表达式语义

表达式通过布尔组合、`all/any/filter/count`、类型化比较、成员判断及正则，组合已有文本/AST 实体事实。
字段只能是谓词当前实体的 `name`、`text`、`path`；不运行任意代码或动态字段。
原有固定断言继续叠加生效，包括声明保留和依赖检查。表达式集合仅包含规则选中的触发实体，
不增加 adapter 本身不具备的语义知识。

类型/形状校验检查全部分支。事实采集必须先完整完成，声明的依赖/来源事实需保持绑定。
完整空集合有明确语义：all 为 true、any 为 false、count 为零；要求至少一个实体时显式增加计数。
解析失败、事实缺失和预算耗尽保持未完成。即使算子未完成，之前固定断言发现的违规仍保留。

## 求值预算与报告

每次求值的嵌套操作共享预算：256 个节点、深度 16、50,000 个实体、32 MiB 事实、100,000 次操作、
32 MiB 字符串扫描及 30 秒截止时间。字面量与正则编译还有独立严格上限。
布尔短路不能隐藏事实缺口或昂贵分支。原有采集预算仍生效，不引入通用代码执行环境。

报告保留原有快照、策略和来源审查绑定。完成求值时输出操作/扫描统计；条件为假产生集合级违规。
混合编写包标识可导出的 Schema 家族及每个 v2 文件的实际契约。
Schema 合法、本地来源审查、独立策略批准及解释是否充分仍是不同的保证。


## 编写并验证表达式

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

规则可以声明多个独立选择的事实集合、绑定变量、引用外层实体，并组合标量 `map`、`distinct`、
`contains`、`subset`、`concat` 和字面量 `replace`。全部声明来源必须完整且绑定同一快照；
缺失事实不能当成空集合。顶层触发与固定断言语义不变。命名文件来源可显式包含未修改的上下文，
但不扩大已采集快照或绕过排除策略；AST 全量选择仍不支持。

[完整 V3 参考与源文件／测试对应示例](../../../skills/qualitygate-cli/references/operators-v3.md)
定义类型、来源、变量作用域、物化预算和诊断。文件对应检查只证明文件关系；行为证据仍由配置的测试
及适用时的 `test_effectiveness` 提供。qualitygate 的职责仍是规则化与规则执行。

---

[上一章：条款覆盖](07-constraint-coverage.md) · [全书目录](../README.md) · [下一章：面向使用者 Agent 的行为契约](09-agent-contracts.md)

主集合与所有命名来源共享同一采集截止时间、实体数量和字节额度。选择器在复制规则事实前
预留额度；语法解析与实体匹配继承剩余时间。预算耗尽时检查保持不完整，并保留已经确认的
固定断言违规。
