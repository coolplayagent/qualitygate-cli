# Qualitygate CLI

[English](README.md) · [完整文档](docs/zh/README.md)

Qualitygate CLI 是面向开发者、CI 和编程 Agent 的仓库质量与任务验收门禁。
它依据仓库策略检查选定的 Git 快照，运行项目已有工具，为交付决策提供可复验的证据。

## 关键竞争力

- **证据绑定具体输入**：结果关联快照、策略和任务，输入变化后不能沿用旧的通过结论。
- **明确区分违规与未完成**：报告缺失、超时或能力不足都会保留为验证缺口。
- **统一规则与现有工具**：组合内置规则、从 `AGENTS.md` 等规范提取的来源绑定项目规则，
  以及现有 lint、测试和覆盖率工具的报告。
- **Agent 验收可追溯**：有界修复反馈最终回到完整快照检查，策略审核与任务验收各有依据。

## 上手步骤

从[项目站点](https://coolplayagent.github.io/qualitygate-cli/)下载 CLI 或匹配的 Agent Skill。
确保 Git 和 `qualitygate` 在 PATH 中可用，然后初始化待检查仓库：

```bash
qualitygate --root /path/to/repository init --format json
```

检查生成的候选 `qualitygate.yaml`，然后验证变更：

```bash
qualitygate --root /path/to/repository check --worktree --profile quick --format json
qualitygate --root /path/to/repository check --worktree --profile full --format json
```

编辑期间用 `quick` 获取反馈，交付前对最终快照运行无路径过滤的 `full` 检查。
退出码 `0` 表示完整通过，`1` 表示阻塞违规，`2` 表示验证未完成。
接下来阅读[安装与首次检查](docs/zh/01-user-guide/01-installation-and-first-check.md)。
