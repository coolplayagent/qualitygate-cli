# 运行时能力与环境预检

[English](../../en/02-reference/06-runtime-and-preflight.md) · [卷目录](README.md)

本功能属于 v0.5.6 之后的未发布开发变更。相同版本号的构建可能不同，必须查询能力 ID
和可执行文件摘要，不能仅凭版本号推断支持情况。

```bash
qualitygate capabilities --format json
qualitygate schema capabilities --format json
qualitygate schema doctor --format json
qualitygate doctor --diff origin/1.8.x-alpha..HEAD --profile full --format json
qualitygate doctor --diff origin/1.8.x-alpha..HEAD --profile full --probe-tools --format json
```

`capabilities` 离线运行，不要求仓库、配置或规则资产，返回 CLI 版本、可执行文件
SHA-256、系统/架构、版本化能力 ID、协议版本和资源限制。摘要区分同版本构建的字节，
不认证发布来源，也不代替开发构建审查。

`doctor` 默认 worktree/full，支持 `--worktree`、`--staged`、`--diff BASE..HEAD`、
工作区 `--base`、`--profile`、`--task`、`--policy-ref` 及与 `check` 共享的四个快照预算选项。
首版不支持远程 MR、路径过滤、feedback 或 decision envelope；MR 仍使用正式流程并明确缺口。

## 配置要求

```yaml
schema_version: 1
requires:
  min_cli_version: '0.5.6'
  capabilities: [config.requires.v1, command.required-env.v1, doctor.static.v1]
checks:
  - id: verify
    argv: [git, --version]
    required_env: [PROJECT_TOOLCHAIN]
    tools:
      - id: git
        argv: [git, --version]
```

`requires` 可省略。最低版本采用语义版本比较，包括预发布版本；能力要求精确匹配运行时
返回的 ID。示例必须保留能力要求，因为已发布的 v0.5.6 没有这些开发功能。空的新字段不改变
旧配置的序列化。可复用规划器和活动策略流程也验证要求。

命令检查与任务 `acceptance[].verification` 可声明 `required_env`。变量名符合
`[A-Za-z_][A-Za-z0-9_]*`，最多 128 个、不重复、每个最多 128 字节。只检查存在且非空，
不打印值，不推断脚本依赖；空白或非 UTF-8 值仍视为非空。正式 `check` 及配对执行也强制检查，
不能靠跳过 doctor 绕过。人工检查不能声明命令环境字段。

解析依次验证 YAML 结构/重复键、显式运行时要求、严格字段/类型和语义。
`runtime.version_too_old`、`runtime.capability_missing`、`config.unknown_field`、
`config.type`、`config.syntax` 分开报告，保留原始错误及可用的字段/行列。字段拼写错误不能
自动解释为版本过旧。旧二进制无法输出新诊断；Skill 结合经核实的兼容表、版本和帮助识别限制。

## 执行与结论边界

1. 复用 check 的活动策略或调用方策略引用、规则目录、任务与 profile，展开所选检查的依赖。
2. 分别检查实际 base、target 和独立策略引用的元数据，应用 exclusions 和受保护输入规则。
   staged 读取 index blob 大小，diff 读取指定提交端点。
3. 预算满足后捕获、物化不可变快照，验证完整策略、工作目录、可执行文件身份、工具输入、
   必需参数和环境变量名。默认不启动任何项目命令。
4. 显式 `--probe-tools` 仅按顺序运行所选 `tools[].argv`。复用正式检查的有界捕获、工具
   身份与输入守卫；整个探针阶段共享 120 秒，单项仍遵循 1–60 秒配置。完成时复核源快照与策略。

超时、输出超限、空/非法版本、非零退出、输入或可执行文件变化都保留为未完成证据。
报告记录退出/时间、工具输入摘要、输出摘要和字节数；不发布探针原始 stdout/stderr 和版本文本，
因为生产者可能打印继承的凭据。临时日志随私有目录清理。正式命令、规则计算和未尝试探针列入
`not_executed`。

报告固定 `scope: preflight`，包含运行时、快照、策略身份、所选和待交付检查、分侧预算、
诊断与 `next_steps`。可选 `next_steps[].command` 使用 argv 数组，直接调用，不作为 shell
字符串解析。退出 0 仅代表请求的预检范围通过；受阻或未完成退出 2。不产生交付门禁或缓存凭证。
预检通过后仍须运行无路径过滤的正式 full check。

## 大仓库模板

默认单文件 2 MiB，上限 8 MiB；每类明细最多 100 条，同时保留完整计数与截断标识。
仅 base 超限也阻止捕获。容量内建议最小整数 MiB；超过 8 MiB 明确说明边界，建议审查排除项，
不修改策略。增加 `--snapshot-max-mib` 总预算不能解决单文件超限；文件数预算包含被排除的清单条目。

仅当元数据或仓库要求证明需要 8 MiB 时使用：

```bash
qualitygate doctor --root . --profile full \
  --diff origin/1.8.x-alpha..HEAD --snapshot-max-file-mib 8 --format json
qualitygate check --root . --profile full \
  --diff origin/1.8.x-alpha..HEAD --snapshot-max-file-mib 8 --format json
```

只审查所选策略中与交付无关的资源排除项；排除内容无法用于构建，受保护验证输入和声明的工具
输入必须保留。`init.snapshot_preflight` 仍是 HEAD/工作区辅助信息，doctor 才检查指定快照的
预检范围。已有有效配置无需重新 init。

## 兼容性与 Shell

以下首次发布版本经过 Git 历史及标签源码核对：

| 能力 | 首次发布版本 | 证据 |
| --- | --- | --- |
| 声明式有界工具版本探针 | v0.3.0 | `a7f9550`，包含于 v0.3.0 |
| 文件契约和报告 ratchet | v0.4.0 | `baa1eb3`，包含于 v0.4.0 |
| `exclude`、单文件预算与 `init.snapshot_preflight` | v0.5.5 | `98c0a66`；v0.5.1–v0.5.4 标签源码中不存在 |
| capabilities、`requires`、`required_env`、doctor | v0.5.6 之后未发布 | 必须核对导出的能力 ID，不能从 `0.5.6` 推断 |

PowerShell 与 Windows 原生 Git Bash/MSYS2 使用 Windows 资产；Linux、WSL 使用 Linux 资产。
Git Bash 可直接调用 `.exe`。带空格路径必须引用，传入原生程序的仓库/资产环境路径通过
`cygpath -m` 转换；仅在该调用中关闭自动参数转换。[Skill](../../../skills/qualitygate-cli/SKILL.md)
提供完整示例，路径行为依据 [MSYS2 文档](https://www.msys2.org/docs/filesystem-paths/)。

Windows CI 执行 Rust 回归和带空格路径下的 PowerShell/Git Bash 直接调用。Linux 测试不能
代替 Windows 实测，需保留实际 Windows job 结果。
