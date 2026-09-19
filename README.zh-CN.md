# Qualitygate CLI

[English](README.md)

Qualitygate CLI 是面向仓库策略和任务验收的 Rust 命令行门禁。它检查选定的 Git 快照，
以有界方式运行项目工具，并为开发者、CI 和编程 Agent 输出可复验的证据。

## 核心特点

- 每个结果都绑定不可变 Git 快照、策略和任务身份。
- 明确区分阻塞违规与未完成执行。
- 统一处理内置规则、Schema 校验的项目规则和外部报告。
- 通过封闭 Schema 与 CLI，把 `AGENTS.md` 等仓库说明中的可执行约束转成绑定来源的结构化规则候选。
- 保持策略、任务、人工验收与签名 provenance 可追溯。
- 为 Agent 提供有界反馈，同时保留最终 full 检查契约。
- 限制时间、并发、文件获取、解析和进程输出资源。

## 最短上手

从[项目站点](https://coolplayagent.github.io/qualitygate-cli/)下载发布版 CLI 或匹配的 Agent Skill，
然后运行：

```bash
qualitygate --root /path/to/repository init --format json
qualitygate --root /path/to/repository \
  check --worktree --profile quick --format json
```

交付前应对最终快照运行无路径过滤的 `--profile full` 检查。退出码 `0` 表示完整通过，`1` 表示
阻塞违规，`2` 表示验证未完成。

## 文档

- [中文文档](docs/zh/README.md)
- [English book](docs/en/README.md)
- [安装与首次检查](docs/zh/01-user-guide/01-installation-and-first-check.md)
- [Schema 驱动的仓库规则](docs/zh/01-user-guide/06-schema-guided-repository-rules.md)
- [CLI 参考](docs/zh/02-reference/01-cli-commands.md)

规范性产品需求位于 [CodeSpec requirements](codespec/requirements/qualitygate-cli.md)；仓库导航由
[CodeSpec Map](codespec/codespec-map.yaml) 与 [Knowledge Map](knowledge/knowledge-map.yaml) 管理。

## 开发

```bash
cargo fmt --all -- --check
cargo check --all-targets --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo llvm-cov --all-targets --all-features --fail-under-lines 90
```

测试、架构、文档、自检、Miri 与 ASan 边界见[贡献者指南](docs/zh/04-contributor-guide/README.md)。

## 许可证

MIT
