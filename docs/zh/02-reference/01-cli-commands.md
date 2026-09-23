# CLI 命令

[English](../../en/02-reference/01-cli-commands.md) · [本卷目录](README.md)

所有仓库命令都接受 `--root`。自动化场景建议使用 `--format json`，面向人工可输出 Markdown。

## 仓库初始化与查看

初始化要求及结构化早期错误见[命令前置条件与恢复](06-command-prerequisites.md)。

```bash
qualitygate --root . init [--with-checks] --format json
qualitygate --root . config --show --format json
qualitygate --root . rules categories --format json
qualitygate --root . rules list [--language rust] [--source all] --format json
qualitygate --root . rules describe RULE_ID --format json
qualitygate --root . rules schema
qualitygate schema command-error
qualitygate --root . rules validate candidate.yaml
qualitygate --root . rules generate --input reviewed-source.md
```

`init`、`rules enable`、`rules disable` 与 `rules configure` 会修改候选仓库配置；list、describe、
Schema 导出和有效配置查看均为只读。

## 检查

```bash
qualitygate --root . check --staged --profile full --format json
qualitygate --root . check --worktree --profile quick --feedback --format json
qualitygate --root . check --diff BASE..HEAD --profile full --task task.yaml
qualitygate --root . check --mr URL --profile full --format markdown
```

每次使用一个快照选择器。`--path` 仅收窄反馈；`--policy-ref` 从已解析提交选择策略，但不能证明
该提交已获批准。`--trust-store` 与 `--evidence-dir` 指向调用方控制的签名证据，通常应位于被检查
仓库之外。

## 自检与试点证据

```bash
qualitygate selfcheck [--fixture NAME] [--rule RULE_ID]
qualitygate pilot seal --input pilot-plan.json --format json
qualitygate pilot authorization-subject --input observations.json --format json
qualitygate pilot acceptance-subject --input observations.json \
  --trust-store /external/trust.json \
  --authorization /external/start.dsse.json --format json
qualitygate pilot summarize --input observations.json --format json
```

selfcheck 将内置 fixtures 与独立编写的 golden 结果比较。pilot 命令只验证并汇总输入记录，不创建
信任密钥、不签名、不批准策略，也不补齐缺失的真实世界证据。

版本特定参数以 `qualitygate --help` 和子命令帮助为准。脚本应同时检查进程退出码与结构化 gate 字段。
