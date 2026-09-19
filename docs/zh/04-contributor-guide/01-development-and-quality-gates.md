# 开发与质量门禁

[English](../../en/04-contributor-guide/01-development-and-quality-gates.md) · [本卷目录](README.md)

仓库自有实现使用 Rust。测试可在文档化 CLI 边界调用 Git 和 fixture 项目已配置的工具。必须使用隔离
临时 Git 仓库，不得修改参考仓库、用户 Git 配置或无关运行时状态。

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

每项行为变化都要更新相关文档和需求—测试证据。按适用情况覆盖通过、违规、未完成、超时、策略变化和
快照不匹配。绝不能削弱失败检查或测试来宣称完成。

所有人工维护的源码、测试、文档和 workflow 文件最多 1,000 行、2 MiB；文档声明的生成 lockfile
例外。Rust 文档门禁解析 Markdown 链接、锚点、引用、HTML 链接和代码围栏；双语书还强制路径同构、
语言切换、目录完整和 `docs/` 根目录整洁。

owner/import 变化后运行架构测试；Skill、发布、版本或文档入口变化后运行包与站点测试。Bazel 是额外
支持的构建面，不替代上述 Cargo 命令。
