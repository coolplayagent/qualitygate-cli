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
cargo llvm-cov --lib --bins --all-features --fail-under-lines 97.01
cargo llvm-cov --all-targets --all-features --fail-under-lines 90
```

单元与集成门禁保持分离。纯 domain 另用 Miri；native 单元测试单独用 ASan。依赖外部工具链或网络的
live producer 测试，未实际运行时不能算入稳定本地套件。

仓库的 `integration` 命令检查与 CI 的 `rust-integration` 作业使用
`cargo test --locked --test '*' --all-features`，CI 同时保留 `--no-fail-fast`。
由 Cargo 展开带引号的目标 glob，使新增集成目标自动进入门禁。单元门禁保持 `--lib --bins`，
benchmark、文档与架构门禁仍分别报告结果。仓库质量测试通过有界、离线的 `cargo metadata`
输出核对策略选择器，拒绝漏选目标、混入单元目标或按测试名过滤。默认运行仍跳过忽略测试，
这些测试需要在文档规定的环境中单独执行。现有 live producer CI 作业继续提供各自工具的执行证据。

Windows 原生 CI 将完整 Cargo 目标清单分成三个作业：`--lib --bins --examples --benches`、
`--test '[a-m]*'` 和 `--test '[!a-m]*'`。带引号的互补 Cargo glob 自动纳入新增集成目标，
无需维护名称列表。各作业保留 `--locked --all-features` 和 30 分钟预算。PowerShell/Git Bash
预检随第一组集成测试执行，该组也会构建原生 CLI。macOS 与本地检查仍使用 `--all-targets`。
调整分组时，逐目标比较各组与完整命令的 `cargo test ... -- --list` 清单，包含空目标与忽略的
测试；并集必须相等，且目标不得重复。
详见 [Cargo 目标选择](https://doc.rust-lang.org/cargo/commands/cargo-test.html#target-selection)。

Windows 步骤的 dev/test profile 关闭调试符号生成以减少 PDB 生成和链接开销，同时显式启用
调试断言与溢出检查。这些独立的
[Cargo profile 设置](https://doc.rust-lang.org/cargo/reference/profiles.html)只作用于该 Windows
步骤；本地默认配置、测试清单、优化级别、Miri 和 ASan 保持不变。作业中断仍属于未完成。

ESLint 验收与 ClawHub 发布步骤使用 action 运行时为 Node 24 的 `actions/setup-node@v7`，
并为工具选择 Node 24。两处均显式设置 `package-manager-cache: false`，保留 v4 关闭工作流
缓存的行为；依赖由临时夹具或全局 CLI 安装步骤准备。
版本变化见[上游迁移说明](https://github.com/actions/setup-node/blob/v7/README.md)。

架构、覆盖率和完整 selfcheck 证据使用 `actions/upload-artifact@v6`，与发布工作流的 Node 24
运行时一致。检查失败后的证据上传仍执行，证据文件缺失仍使上传失败。静态 Pages 工作流使用
`configure-pages@v6`、`upload-pages-artifact@v5` 和 `deploy-pages@v5`；上传保留 `path: site`，
并显式排除隐藏文件。本地验证站点/打包契约后，还需分别检查实际 artifact 和 Pages 工作流结果；
本地测试不能证明远程上传或部署成功。

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

## 面向场景的单元覆盖率

仅 UT 的行覆盖率门槛为 **97.01%**，使用 `--lib --bins --all-features`，保证严格高于 97%。
该门禁独立于集成测试运行，并保留单独的 LCOV 产物；原有 all-target 90% 门禁仍然必需。
不得把集成测试 profile 合并进 UT 测量，也不得排除未覆盖的生产模块来满足门槛。

单测针对通过、违规、执行未完成、超时、策略变化和快照不匹配，断言可观察的决策、保留证据和输入完整性。
原生边界场景使用临时 Git 仓库和有界的可执行协议替身。这些单测不能证明真实编译器、供应商、Maven、
Bazel 或端到端行为，独立集成门禁仍需通过。selfcheck 单元语料验证内存中的求值场景，原生工作流夹具
仍属于独立的集成证据。

慢 UT 需排除编译时间，并使用单测试线程复测。预算场景矩阵可以向同一个私有解析器
注入小预算，但每种受影响格式的公开入口必须保留真实 16 MiB 边界的接受与拒绝测试；
不得通过跳过场景或降低生产限制提速。

保存的通过报告不能单独证明进程成功结束。必须分别保留 CLI 与外层包装器退出状态：
shell 状态 143 通常表示 SIGTERM，但不能据此识别信号发送方。子进程组终止必须保持为
未完成检查和 CLI 退出码 2，不得终止 CLI 或调用者。物化快照需使用独立的 Cargo target
目录；与其他检出目录共享缓存可能复用包含已删除编译期资源路径的可执行文件。
