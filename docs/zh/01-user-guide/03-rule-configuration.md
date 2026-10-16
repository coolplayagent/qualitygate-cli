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

## 4. 验证选中的策略

```bash
qualitygate doctor --worktree --profile full --format json
qualitygate check --worktree --profile full --format json
```

确认计划包含预期检查，查看待执行项、违规和证据缺口。修改策略或规则后需要重新验证；使用
staged/diff 检查时，审核后的配置必须位于所选快照中。将报告与采用的策略身份一起保留。

---

[上一章：快照与检查流程](02-snapshots-and-check-workflow.md) · [全书目录](../README.md) · [下一章：Schema 驱动的仓库规则](06-schema-guided-repository-rules.md)
