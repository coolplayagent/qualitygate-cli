# 规则配置

[English](../../en/01-user-guide/03-rule-configuration.md) · [本卷目录](README.md)

仓库策略决定交付需要哪些规则和命令。从 `init` 生成的候选开始，先查看，再调整选择和阈值。
下列命令均在仓库根目录执行。

## 1. 查看规则目录与有效策略

```bash
qualitygate rules categories --format json
qualitygate rules list --source all --format json
qualitygate rules describe line-ending --format json
qualitygate config --show --format json
```

规则目录也包含未选择的内置包和项目规则。阅读各项的 `enabled`、`source`、`language`、能力需求、
严重级别和参数。查看或分配类别不会启用检查；对照有效策略，才能确认实际检查采用了哪些规则。

## 2. 选择实现方式

| 仓库要求 | 配置方式 |
| --- | --- |
| 已支持的提交、文本或语法约定 | 阅读定义后选择对应内置规则。 |
| 绑定规范来源的文件、命名、标记或有界断言 | 在 `qualitygate/rules` 中维护项目规则。 |
| lint、测试或覆盖率命令 | 配置命令检查及对应报告适配器。 |
| 现有分析器债务不能增加 | 配置报告棘轮，重新执行基线。 |
| 需要人工判断的验收项 | 使用明确的人工验收与外部签名证据。 |

文件契约和报告棘轮是配置方式，不是额外的内置规则 ID。缺少语义事实时保留缺口；文本命中不能证明
调用解析、数据流或运行时正确性。

## 3. 修改并审核候选

例如，选择已有的换行规则是一项显式变更：

```bash
qualitygate rules enable line-ending
qualitygate config --show --format json
```

`rules configure RULE_ID --set key=value` 只能使用该规则定义支持的参数。写入前校验完整候选，
再原子替换；参数无效、ID 重复或规则包不可读都会报错。有效候选仍需经过团队策略审核和采用流程。
关闭必需检查不能算作修复失败变更。

编写项目规则请继续阅读[Schema 驱动的仓库规则](06-schema-guided-repository-rules.md)。
命令报告与采用边界分别见[报告参考](../02-reference/03-projects-and-tool-reports.md)和
[策略生命周期](../02-reference/04-policy-task-and-signed-evidence.md)。

## 4. 按变更语言和项目选择命令

命令检查只要被 profile 选中，默认就会执行；声明 `when_changed` 后才按变更判断适用性。
只有明确了解命令受哪些输入影响时才配置选择条件。顶层 `languages` 是语言清单，不会隐式跳过命令。

```yaml
checks:
  - id: service-tests
    kind: command
    argv: [cargo, test, --package, service]
    when_changed:
      languages: [rust]
      paths: ['crates/service/**']
      inputs: ['crates/shared/**', 'fixtures/service/**']
```

对每个变更路径，已提供的 `languages` 与 `paths` 必须同时匹配；缺省或空列表表示该维度不限制。
`inputs` 是独立于语言和项目路径的附加 OR 条件；只提供 inputs 时，仅按该输入列表匹配。
应列出会影响命令的共享源码、生成输入和测试数据。路径使用区分大小写、相对仓库根目录的 glob，
不相对 `cwd`。选择器至少包含一个非空列表；未知语言、重复语言和非法 glob 都会报错。
支持的语言名为 `java`、`python`、`typescript`（含 JavaScript）、`go`、`rust`、`shell`、
`c` 和 `cpp`；Shell shebang 可以识别没有扩展名的输入。
`.h` 头文件变化会保守地同时匹配 `c` 和 `cpp` 命令选择器。

调度读取完整采集变更集，删除、重命名或语言改变同时考虑旧、新路径及语言。`--path` 只过滤反馈，
不会缩小命令选择范围。策略、受保护验证资产、工具输入及已识别的 manifest、锁文件或构建配置变化时，
保守地选中计划内全部命令。可复用核心的 repository 范围也执行全部计划命令。适用检查的依赖会
递归执行，包括选中规则需要的命令。人工验收和任务契约 verification 不允许声明 `when_changed`。

[全局输入清单](../../../src/qualitygate/config/change_selection.rs)覆盖 Cargo、Maven/Gradle、
Python 包与测试配置、Node/TypeScript、Go、CMake/Make 和 Bazel 输入。例如 `Cargo.lock`、
`package.json`、`requirements*.txt`、`tsconfig*.json`、`.bzl` 和 `.cmake` 变化，即使位于
所声明项目之外，也会保守触发。项目专有输入仍需显式声明；该清单不等同于依赖图。

未匹配命令仍保留在报告中，标为 `not_applicable`、`not_run`、`skipped`，附带明确原因和
`metadata.change_selection`。这一判断先于该命令的环境检查、工具探针和工作区准备。
适用命令的失败、超时、报告缺失及快照不匹配仍阻止完整通过。选择条件属于策略审核内容；
未声明某个依赖并不能证明它与命令无关。
全部选中检查均被跳过时，门禁仍为未完成；仅有 skipped 命令不能证明交付通过。

选择器决定是否执行已配置命令，不改写 `argv`、不推断测试目标，也不从不可变执行快照中移除文件。

## 5. 验证选中的策略

```bash
qualitygate doctor --worktree --profile full --format json
qualitygate check --worktree --profile full --format json
```

确认计划包含预期检查，查看待执行项、违规和证据缺口。修改策略或规则后需要重新验证；使用
staged/diff 检查时，审核后的配置必须位于所选快照中。将报告与采用的策略身份一起保留。

---

[上一章：快照与检查流程](02-snapshots-and-check-workflow.md) · [全书目录](../README.md) · [下一章：Schema 驱动的仓库规则](06-schema-guided-repository-rules.md)
