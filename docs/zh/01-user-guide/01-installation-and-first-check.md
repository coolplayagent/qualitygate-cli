# 安装与首次检查

[English](../../en/01-user-guide/01-installation-and-first-check.md) · [本卷目录](README.md)

本章带你在已有 Git 仓库中完成安装、初始化和首次完整检查。需要准备 Git，以及仓库检查所依赖的
工具链。下文的 `/path/to/repository` 均替换为待检查仓库路径。

## 1. 安装并确认运行时

从[项目站点](https://coolplayagent.github.io/qualitygate-cli/)下载适合操作系统和架构的 CLI 压缩包。
完整解压，保持可执行文件与规则资源的相对位置，并把可执行文件所在目录加入 PATH。
接入 Agent 时使用匹配的 Skill 压缩包及其内置可执行文件。

```bash
qualitygate --version
qualitygate capabilities --format json
```

能力 ID 用于确认可用接口；仅凭版本号不能区分开发构建。从源码构建的步骤见
[贡献者指南](../04-contributor-guide/01-development-and-quality-gates.md)。

## 2. 初始化并检查候选配置

```bash
qualitygate --root /path/to/repository init --format json
qualitygate --root /path/to/repository config --show --format json
```

`init` 发现语言、构建清单、可用检查和能力缺口，并在缺少配置时创建候选 `qualitygate.yaml`。
检查选中的规则和 profile。需要加入发现的命令候选时，在团队策略采用流程内运行
`init --with-checks`；该命令不会安装或执行这些工具。已有有效配置时可直接查看，无需再次初始化。

生成候选不等于批准策略。通过结论只覆盖已配置检查：初始化本身不能保证项目测试或覆盖率工具已被
选中。需要适配仓库时，继续阅读[规则配置](03-rule-configuration.md)。

## 3. 检查前置条件并获取反馈

```bash
qualitygate --root /path/to/repository doctor --worktree --profile full --format json
qualitygate --root /path/to/repository check --worktree --profile quick --format json
```

默认 `doctor` 只检查前置条件，不执行项目命令。处理报告中的缺口后再运行检查。
`quick` 对捕获的工作区字节执行该 profile 选中的检查，包括符合条件的未跟踪文件，适合编辑时使用；
尚未执行的交付检查仍会出现在报告中。

## 4. 验证最终快照

```bash
qualitygate --root /path/to/repository check --worktree --profile full --format json
```

| 退出码 | 含义 | 下一步 |
| --- | --- | --- |
| `0` | 所选范围完整通过 | 保存报告、警告和证据边界。 |
| `1` | 发现阻塞违规 | 修复问题并重新检查。 |
| `2` | 验证未能完成 | 补齐前置条件或证据后重试。 |

交付应使用无路径过滤、没有待执行交付检查的 full 报告。之后再修改输入，必须重新检查。
下一章介绍[快照、报告身份和交付范围](02-snapshots-and-check-workflow.md)。

若采集因超大文件或不支持的输入停止，先查阅[运行时与预检](../02-reference/06-runtime-and-preflight.md)，
再决定预算或排除范围。保留报告中的未完成原因，不能把中断的检查视为通过。

---

[全书目录](../README.md) · [下一章：快照与检查流程](02-snapshots-and-check-workflow.md)
