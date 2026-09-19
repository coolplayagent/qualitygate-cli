# 安装与首次检查

[English](../../en/01-user-guide/01-installation-and-first-check.md) · [本卷目录](README.md)

Qualitygate CLI 按仓库策略检查指定 Git 快照；提供任务契约时还会合并任务验收要求。
运行时需要 Git，策略中配置的项目命令还需要各自的工具链。

## 安装

可从[项目站点](https://coolplayagent.github.io/qualitygate-cli/)下载发布版二进制或版本匹配的
Agent Skill。贡献者也可在当前源码中构建：

```bash
cargo build --locked
cargo run -- --version
```

发布的 Skill 自带匹配版本的可执行文件和规则资产，不应静默替换为无关源码树构建的程序。

## 发现候选策略

在待检查仓库中运行：

```bash
qualitygate --root /path/to/repository init --format json
qualitygate --root /path/to/repository init --with-checks --format json
```

`init` 清点语言、构建清单、已有命令、分析器报告与能力缺口。它可以生成候选
`qualitygate.yaml`，但不会批准候选策略、推断豁免或代表团队完成采用。启用前应审核建议命令、
项目识别结果和不支持的形态。

发现过程有明确边界。文件不可读、清单格式错误、生态不支持或预算耗尽都保留为能力缺口，
不会静默视为成功。

## 首次检查

```bash
qualitygate --root /path/to/repository \
  check --worktree --profile quick --format json
```

命令捕获已跟踪改动和范围内未跟踪文件，物化不可变执行输入，再输出结构化诊断。
`quick` 可用于修复反馈，但不能证明可交付；交付前必须对最终快照执行无路径过滤的
`--profile full` 检查。

结果明确区分三种状态：完整通过、完整执行但存在阻塞违规，以及验证未完成。解释结论时应同时
查看快照、策略、范围、待执行检查、warning 和证据引用。
