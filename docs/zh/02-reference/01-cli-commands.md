# CLI 命令

[English](../../en/02-reference/01-cli-commands.md) · [本卷目录](README.md)

通过 `qualitygate --help` 和子命令帮助确认所装运行时的精确选项。仓库命令支持 `--root`，下例均
在仓库根目录执行。自动化使用 `--format json`，同时检查退出码与报告字段。大写标识符和示例路径
需要替换为实际输入。

## 初始化与查看

```bash
qualitygate init --format json
qualitygate config --show --format json
qualitygate capabilities --format json
qualitygate doctor --worktree --profile full --format json
```

`init` 创建缺失的候选配置，`init --with-checks` 还会加入命令候选。`config --show` 和
`capabilities` 只读；默认 `doctor` 不运行项目命令，`doctor --probe-tools` 会执行声明的工具探针。
诊断、兼容性和初始化行为见[运行时与预检](06-runtime-and-preflight.md)。

## 规则发现与编写

```bash
qualitygate rules categories --format json
qualitygate rules list --language rust --source all --format json
qualitygate rules describe RULE_ID --format json
qualitygate rules schema --format json
qualitygate rules source --document AGENTS.md --section "Quality contract" --format json
qualitygate rules validate candidate.yaml --format json
qualitygate rules generate --input candidate.yaml --format json
```

`generate` 写入结构化规则候选，不会把任意自然语言直接变成已采用规则。`rules enable`、
`rules disable` 和 `rules configure` 修改候选策略；查看、描述、来源绑定、Schema 导出与验证均只读。
详见[规则参考](02-rules.md)和[编写教程](../01-user-guide/06-schema-guided-repository-rules.md)。

## 快照检查

```bash
qualitygate check --staged --profile full --format json
qualitygate check --worktree --profile quick --feedback --format json
qualitygate check --diff BASE..HEAD --profile full --task task.yaml --format json
qualitygate check --mr URL --profile full --format markdown
```

选择一种快照选择器，省略时使用工作区。`--path` 缩小反馈范围。`--policy-ref` 选择解析后的策略提交，
但不证明它已获批准。`--trust-store` 和 `--evidence-dir` 提供调用方控制的签名证据输入。
检查命令会执行策略选中的项目工具。结果含义见[退出码与限制](05-exit-codes-and-resource-limits.md)。

## Schema 与 Agent 集成

```bash
qualitygate schema capabilities --format json
qualitygate schema doctor --format json
qualitygate schema constraint --format json
qualitygate schema operator-rule --format json
qualitygate schema project-rule-family --format json
qualitygate schema agent-obligation --format json
qualitygate agent-contract --worktree --role implementer --format json
qualitygate agent-run --help
qualitygate check --worktree --profile full --export-delivery /external/delivery --format json
```

使用前查询能力 ID。Schema 导出和契约投影只读，投影不执行检查。`agent-run` 会启动配置的 Agent，
可能修改其检出目录并写入证据；交付导出在仓库外写工件。必需输入与限制见
[Agent 契约](09-agent-contracts.md)和[宿主接入](10-agent-host-and-delivery.md)。

## 自检与试点评估

```bash
qualitygate selfcheck --format json
qualitygate pilot seal --input pilot-plan.json --format json
qualitygate pilot authorization-subject --input observations.json --format json
qualitygate pilot acceptance-subject --input observations.json --trust-store /external/trust.json --authorization /external/start.dsse.json --format json
qualitygate pilot summarize --input observations.json --format json
```

selfcheck 执行随包 fixtures 并与独立 golden 对照。试点命令校验输入的计划和记录，不代签主题，
也不补造观察数据。详见[自检](../04-contributor-guide/02-selfcheck-and-fixtures.md)和
[试点评估](../04-contributor-guide/04-pilot-evaluation.md)。

---

[上一章：在真实仓库评估采用效果](../01-user-guide/05-real-repository-pilot.md) · [全书目录](../README.md) · [下一章：内置与自定义规则](02-rules.md)
