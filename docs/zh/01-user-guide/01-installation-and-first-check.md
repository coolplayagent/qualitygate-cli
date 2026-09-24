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

未初始化时，`config --show` 返回未完成和退出码 2。JSON 保留 `schema_version`、
`gate`、`verification`，仅在能确定可靠操作时增加 `next_steps`。本地策略缺失时，
引导命令包含所选 `--root` 和自定义 `--config`；应审核生成的候选策略后再重试。
`next_steps[].command` 是包含可执行文件及原始参数的数组，应直接创建进程执行，
无需 shell 解析。table/Markdown 标明展示命令适用的 shell（Windows 为 PowerShell，
其他平台为 POSIX shell）；`cmd.exe` 调用方应使用结构化参数，不应复制 PowerShell 展示命令。
所选快照缺少策略时，提示将审核后的文件纳入快照；显式指定 `--policy-ref` 时仅提示
选择包含策略的引用，即使本地没有配置也不建议初始化。其他读取、路径和解析错误不会建议执行 `init`。Windows
普通路径在安全可转换时不展示内部 `\\?\` 前缀。`init` 还输出建议性的
`snapshot_preflight`，检查 HEAD 与符合条件的工作区文件元数据，包括被 discovery 忽略但
已跟踪的文件。过大文件和不支持条目各最多列出 100 项，并提供总数、截断状态与预检未完成原因。
这不代表任意 diff/MR 端点、暂存内容、总预算或工具链已经通过检查。

发现超过默认 2 MiB 的历史文件时，按建议给 `check` 添加
`--snapshot-max-file-mib N`（1–8 MiB）。这是调用者的采集容量，不是策略豁免。
`--with-checks` 只加入待审查的命令候选，不安装依赖或运行命令。用户授权接入后应审核候选；
正常调整采集预算重试不需要修改仓库规则。

发现过程有明确边界。文件不可读、清单格式错误、生态不支持或预算耗尽都保留为能力缺口，
不会静默视为成功。

## 首次检查

存量仓库先完成裁剪复核：运行 `init --with-checks --format json`，审核候选命令及预检；
在已授权范围内为历史资源添加 YAML 顶层 `exclude`，例如 `exclude: ["gitbook/images/**"]`。
再次运行 `init --format json`，检查 `excluded_file_count`、`excluded_paths`、剩余大文件及
预检完整性。不要自动豁免所有大文件；排除会使工具无法读取对应资源。

将配置纳入所选暂存区或提交后，执行对应 `check --profile full`。
若构建依赖被排除，缩小模式并重检；保留范围与排除证据，不将裁剪后通过表述为全仓通过。

```bash
qualitygate --root /path/to/repository \
  check --worktree --profile quick --format json
```

命令捕获已跟踪改动和范围内未跟踪文件，物化不可变执行输入，再输出结构化诊断。
`quick` 可用于修复反馈，但不能证明可交付；交付前必须对最终快照执行无路径过滤的
`--profile full` 检查。

结果明确区分三种状态：完整通过、完整执行但存在阻塞违规，以及验证未完成。解释结论时应同时
查看快照、策略、范围、待执行检查、warning 和证据引用。
