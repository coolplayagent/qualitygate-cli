# 项目与工具报告

[English](../../en/02-reference/03-projects-and-tool-reports.md) · [本卷目录](README.md)

项目适配器提供不能从单个源码文件安全推断的事实。Maven 分析解析模块、声明依赖、producer 关系和
编译使用证据；Python 分析比较已安装 distribution 与声明 requirement，并保留解释器和环境身份。
清单缺失、模块归属不明确、工件无法解析或 requirement 语法不支持时保持未完成。

## 统一报告契约

外部检查声明固定命令、工作目录、超时、认可状态、工具/版本证据、报告路径、解析 profile 和模式。
报告只能从快照物化执行中读取，并受文件大小、数量、诊断数和解析时间限制。缺失、过期、格式错误、
截断或快照不匹配的报告不能静默通过。

归一化结果保留工具与规则 ID、严重级别、消息、URI、位置、指纹、计数、原报告摘要、命令证据和
完整性。退出码与报告内容一起判定；认可的 producer 退出码不能覆盖 fatal 或 incomplete 报告。

## 覆盖率与静态分析

覆盖率在声明 profile 内支持原生 coverage.py JSON、Cobertura XML、JaCoCo XML 与 LCOV。阈值基于
covered/total 行事实，并保留零计数。空报告、内部计数矛盾、源码映射缺失、重复冲突条目或解析超限
均为未完成。高覆盖率不能证明行为正确。

SARIF 保留 run、工具版本、规则、result kind、消息、位置与 partial fingerprint。ratchet 会在两个
不可变快照上重新运行相同分析器，再按配置比较 `(tool, rule)` 计数或稳定身份。抑制、缺失和仅基线
存在的结果保持可区分。

参考棘轮覆盖 Clippy、ESLint、golangci-lint、Ruff、GCC/Clang analyzer SARIF、Checkstyle、PMD 与
SpotBugs。各 producer 的完成标志、文件清单、错误记录、路径映射和版本一致性属于完整性契约。

## 兼容性、Bazel 与增量语义

Java 兼容性检查构建成对快照，并调用配置的二进制/源码兼容工具。任一侧构建失败、API 面无法解析或
工具未执行都属于未完成。

Bazel 是使用 Bzlmod 且与 Cargo 依赖对齐的额外可复现构建面，不替代 Cargo 必需门禁。缓存状态、
lockfile 行为、平台和选中 target 都属于证据。

覆盖率元数据 `<report>:mode` 记录实际测量模式：delivery 即使配置为 `full`，也记录
`changed_lines`；`<report>:configured_mode` 保留配置值。可复用核心的 repository 评估使用配置的模式，
计数和阈值判定均遵循实际测量模式。

`new_diagnostics` 先用当前保留的未改动诊断消耗基线同标识计数，再筛选交付诊断。
保留历史诊断并在变更行新增相同诊断时，新增实例仍会被报告；单纯移动已有诊断不会被当成新增。

增量模式必须声明含义：`new_diagnostics`、`changed_lines`、`affected_scope` 或 `full`。文件发生变更
不代表其历史诊断全部变成新增问题。缺少可比基线时，必需 ratchet 保持未完成，除非受信策略事先
允许且明确披露 full 回退。
