# 快照与检查流程

[English](../../en/01-user-guide/02-snapshots-and-check-workflow.md) · [本卷目录](README.md)

应选择与交付面一致的一种快照选择器：

```bash
qualitygate check --staged --profile full --format json
qualitygate check --worktree --profile quick --format json
qualitygate check --diff BASE..HEAD --profile full --format json
qualitygate check --mr https://github.com/owner/repository/pull/123 --format json
```

- `--staged` 读取索引区字节，而不是工作区副本。
- `--worktree` 捕获当前工作区以及符合范围的未跟踪文件。
- `--diff` 比较两个已解析提交。
- `--mr` 解析 GitHub 或 GitLab 合并请求，并记录实际 base、head、merge base、平台与获取证据。

MR 使用调用方提供的凭据和端点。认证失败、网络问题、ref 漂移、限流或不支持的平台响应都属于
未完成执行，不能解释为干净结果。

## 计划、执行与决策

CLI 从选中快照（或显式策略引用）严格加载配置，验证可选任务契约，映射改动和项目事实，
再生成带依赖关系的检查计划。命令在快照的一次性物化目录中运行；标准输出、标准错误、状态、
耗时、工具版本和报告工件均受限采集。

`quick` 和 `--path` 会主动省略部分工作，并列出未执行的交付检查。只有无路径过滤的 `full`
报告，同时满足范围为 `repository` 或 `task`、待执行交付项为空且 gate 完整，才可给出已配置的
交付结论。

## 大型仓库

`--diff` 和 `--mr` 在计算变更前获取完整 base/head 树；`--path` 只在采集后过滤反馈。
语言选择和规则路径条件不排除快照输入，所以只改 README 也可能遇到其他目录的历史大文件。
这种情况是采集未完成（退出码 2），不是 repository-scope 规则违规。

单文件默认预算为 2 MiB。对已审查的大文件，可传 `--snapshot-max-file-mib 8`（范围 1–8），
完整保留文件字节与快照身份。`--snapshot-max-mib` 只控制总预算，单独提高它无效。
初始化预检和错误信息会在可支持时建议足够的单文件预算。超过 8 MiB 仍然不支持；
不会隐式忽略文件、降低严重度或把部分检查标为完整通过。重检命令保留单文件预算。

获取快照时会先检查 Git 对象大小，并用有界批次和并发读取。调用方可调整总字节、worker 和
截止时间预算，而不改变策略含义：

```bash
qualitygate check --worktree --profile full \
  --snapshot-max-mib 512 --snapshot-jobs 8 --snapshot-timeout-secs 300
```

符号链接、submodule、未解决的索引冲突、超大文件、缺失 Git 对象或预算耗尽都会失败关闭。
路径过滤仅缩小反馈，不能把局部执行变成完整交付证据。

应将 JSON 报告与其证据引用一并保存。源码、策略、任务输入或外部报告发生变化后，必须重新选择
并检查快照。
