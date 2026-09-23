# 命令前置条件与恢复提示

[English](../../en/02-reference/06-command-prerequisites.md) · [本卷目录](README.md)

命令按实际依赖检查前置条件。合法的手工配置也视为已初始化，不要求存在 init 执行记录。
恢复提示不会自动执行初始化、工具安装、文件暂存或信任材料修改。

| 命令 | 所需输入 |
| --- | --- |
| 帮助、版本、Schema 导出、`policy evaluator` | 自身内置数据或可执行文件身份；无需仓库配置。 |
| `selfcheck` | fixture 工具、资产与临时空间；无需目标仓库策略。 |
| `init` | 可访问目录、发现资产和候选写入条件；Git 预检仍为建议性结果。 |
| `rules list/describe/categories/context` | 规则资产；默认配置缺失时允许发现，显式自定义配置或策略引用必须存在。 |
| `rules source/validate/generate` | 指定文件、Schema、来源绑定，发布需要写入条件；已有激活策略认证仍适用。 |
| `config --show`、本地规则/类别修改 | 展示需要有效策略；修改需要本地候选，先检查配置再加锁，并在锁内重读。 |
| `check` | Git、所选策略/快照、任务与计划，再验证每项选中检查的实际依赖。 |
| 归档、候选及生命周期操作 | 对应归档对象、状态与授权；保留证据的入口允许创建归档。 |
| `pilot`、`judgment` | 指定输入、输出存储及适用的外部证据或服务。 |

本地配置缺失返回 `repository_not_initialized`，提供保留 `--root` 和 `--config` 的完整 init argv。
本地已有配置但暂存区缺失返回 `policy_snapshot_missing`，提示审核后暂存。
历史策略缺配置时应选择正确引用；初始化工作区不能改变历史快照。
有效激活策略不因本地 YAML 缺失而失效；认证失败不会回退到本地候选。

工具与工作目录在依赖检查完成后验证，因为它们可能由前序检查生成。
只探测当前 profile 选中的检查；工具失败保留原有必需/可选 gate 语义。
报告在生成后检查新鲜度、完整性及快照身份，前置检查不代表执行成功。

## 机器输出契约

使用 `qualitygate schema command-error` 导出 `urn:qualitygate:command-error:1`。
早期运行失败包含 `kind: command_error`、`schema_version: 1`、原有 incomplete `gate`、
`verification` 和结构化 `issues`。每项包含 `code`、`phase`、`message`、可选的
`resource`、`check_id`、`cause`，以及 `next_actions`。
恢复动作是含说明的 `instruction`，或含说明及完整 `argv` 数组的 `command`。
argv 必须按独立参数传递，不拼接成 shell 程序。退出码仍为 2。

即使使用 `--feedback` 或 `--envelope`，客户端也需识别这一早期错误分支。
没有生成的 run ID、快照身份或报告位置不会被伪造；完整报告和 envelope 协议保持不变。
反馈字节上限同样约束早期错误，优先省略过长原因；完整 argv 无法容纳时改用说明型动作。
Clap 参数语法错误保留现有用法输出与退出码。

单项检查的结构化问题保存在 `metadata.prerequisites`，feedback 可定位完整报告。
常见错误码包括 `config_invalid`、`input_missing`、`input_unreadable`、`git_unavailable`、
`git_repository_invalid`、`git_ref_unavailable`、`snapshot_unavailable`、`rule_assets_unavailable`、
`plan_invalid`、`tool_unavailable`、`tool_probe_failed`、`workspace_unavailable`、`storage_unavailable`、
`lock_unavailable`、`archive_unavailable`、`evidence_invalid`、`policy_authentication_failed` 和
`remote_unavailable`。未分类异常保留 `unknown_error` 及原因，不依赖操作系统报错文字分类。
