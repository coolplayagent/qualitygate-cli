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
报告，同时满足范围为 `delivery`、`repository` 或 `task`、待执行交付项为空且 gate 完整，才可给出已配置的
交付结论。

## 大型仓库

所有 check 默认 `--scope delivery`，独立于 quick/full 检查组合。行级诊断只纳入新增／修改行，
无行号的文件诊断按变更文件判定；删除、重命名、二进制与权限变更保留文件级记录。
无文件位置的命令失败、测试失败、超时和证据缺失仍影响门禁。
`--scope repository` 显式恢复仓库范围检查。空交付或交付范围通过均不代表全仓质量通过。

构建和测试仍可读取未排除的依赖上下文。`--diff` 和 `--mr` 获取两端未排除的输入；
`--path` 与语言条件不替代内容排除。未排除的大文件仍可能导致采集未完成（退出码 2）。

在有效 `qualitygate.yaml` 中声明精确排除项：

```yaml
exclude:
  - "gitbook/images/**"
  - "legacy/demos/**"
```

模式为大小写敏感、使用 `/` 分隔的仓库相对 glob，对已跟踪文件也生效。
不支持绝对路径、父目录穿越、反向包含或 `.qualitygateignore`；`.gitignore` 不排除已跟踪输入。
匹配内容不读取、不检查，也不提供给构建和测试。有效策略、任务、自定义规则、规则来源和
受保护验证资产不能排除。若排除导致构建缺资源，需要缩小范围并重新执行。
排除配置来自所选策略，本地未提交编辑不能改变远端 MR 的策略。

单文件默认预算为 2 MiB。对已审查的大文件，可传 `--snapshot-max-file-mib 8`（范围 1–8），
完整保留文件字节与快照身份。`--snapshot-max-mib` 只控制总预算，单独提高它无效。
初始化预检会建议单文件预算或明确的裁剪配置。超过 8 MiB 的文件必须被有效策略明确排除，
否则仍未完成。报告 `selection` 记录范围、排除项、变更文件／行数、空交付和执行上下文摘要；
范围与排除配置绑定验证身份，重检保留 scope 和采集预算。

测试有效性检查在组合旧生产代码与新测试时沿用相同的单文件容量与有效排除配置。
受保护的 `policy candidate validate` 则从外部验收套件读取 `budget.snapshot_max_file_mib`
（默认 2，范围 1–8），供两套策略共同使用。调整该字段会改变套件摘要，需要匹配的外部信任；
普通 check 参数不能覆盖受保护套件的预算。

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
