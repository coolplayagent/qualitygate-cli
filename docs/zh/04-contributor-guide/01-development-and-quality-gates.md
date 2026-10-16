# 开发与质量门禁

[English](../../en/04-contributor-guide/01-development-and-quality-gates.md) · [本卷目录](README.md)

仓库自有实现使用 Rust。测试可在文档化 CLI 边界调用 Git 和 fixture 项目已配置的工具。必须使用隔离
临时 Git 仓库，不得修改参考仓库、用户 Git 配置或无关运行时状态。

## 从源码构建

仓库的 `rust-toolchain.toml` 选择贡献者工具链。安装其中声明的组件后，构建并查看本地可执行文件：

```bash
cargo build --locked
cargo run -- --version
```

这是开发构建，与安装的发布版或 Skill 运行时分别标识。

## 必需检查

运行稳定本地契约：

```bash
cargo fmt --all -- --check
cargo check --all-targets --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo llvm-cov --all-targets --all-features --fail-under-lines 90
```

单元与集成门禁保持分离。纯 domain 另用 Miri；native 单元测试单独用 ASan。依赖外部工具链或网络的
live producer 测试，未实际运行时不能算入稳定本地套件。

Windows 原生 CI 在 30 分钟作业预算内执行同一条完整 Cargo 测试命令。该步骤的 dev/test profile
关闭调试符号生成以减少 PDB 生成和链接开销，同时显式启用调试断言与溢出检查。这些独立的
[Cargo profile 设置](https://doc.rust-lang.org/cargo/reference/profiles.html)只作用于该 Windows
步骤；本地默认配置、测试清单、优化级别、Miri 和 ASan 保持不变。作业中断仍属于未完成。

每项行为变化都要更新相关文档和需求—测试证据。按适用情况覆盖通过、违规、未完成、超时、策略变化和
快照不匹配。绝不能削弱失败检查或测试来宣称完成。

所有人工维护的源码、测试、文档和 workflow 文件最多 1,000 行、2 MiB；文档声明的生成 lockfile
例外。Rust 文档门禁解析 Markdown 链接、锚点、引用、HTML 链接和代码围栏；双语书还强制路径同构、
语言切换、目录完整和 `docs/` 根目录整洁。

owner/import 变化后运行架构测试；Skill、发布、版本或文档入口变化后运行包与站点测试。Bazel 是额外
支持的构建面，不替代上述 Cargo 命令。


## 维护文档

本书使用普通 Markdown，无需生成器。中英文路径、含义、语言切换和章节导航应保持对应。
教程负责操作顺序，参考手册负责精确契约，架构负责设计依据；其他章节链接到主要说明，避免复制。
清除已经完成的路线图表述，同时保留仍受支持的兼容性和证据边界。

```bash
cargo test --test quality --all-features
```

该目标检查文档、架构、站点入口和 Skill 包契约，与前面的稳定检查共同使用。
验证命令示例时使用隔离 Git 仓库，并确认所选运行时，不修改用户 Git 配置。

## 需求与仓库导航

[CodeSpec 需求](../../../codespec/requirements/qualitygate-cli.md)是规范性产品契约，
[证据索引](03-requirements-to-test-evidence.md)将需求关联到测试。
历史实施记录留在 [CodeSpec 设计](../../../codespec/design/README.md)。
[CodeSpec Map](../../../codespec/codespec-map.yaml) 与
[Knowledge Map](../../../knowledge/knowledge-map.yaml)管理仓库导航。
项目采用 MIT 许可证。

---

[上一章：规则能力与 Agent 行为契约](../03-architecture/07-rule-capabilities-and-agent-contracts.md) · [全书目录](../README.md) · [下一章：自检与 fixtures](02-selfcheck-and-fixtures.md)
