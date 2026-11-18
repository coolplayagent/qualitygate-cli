# 项目自建 ASan 检查器

将本目录的 `tools/asan/` 复制到被检项目，在 `suites.json` 填写套件的原生 CTest 测试名、测试可执行文件，以及应插桩的目标文件清单。路径相对于 Ninja 构建目录。示例对应 CMake 目标 `native`、两个翻译单元和测试 `memory`。将示例策略的检查、full profile 条目和验证资产合并到项目策略。清单属于项目验证资产，需随目标及依赖变化评审。

## 运行

参考环境为 Linux、Clang/Clang++ 及其 ASan runtime、Ninja、CMake/CTest 3.29 以上，以及支持 Rust 2024 的 Cargo 工具链和锁定依赖。将匹配的 llvm-symbolizer 放入 PATH 可得到源码栈。项目直接使用 CTest 启动本地测试程序。需要自定义工具链、交叉编译、测试包装命令或其他构建系统的项目可替换这个驱动并复用同一报告协议。

```sh
cargo run --locked --manifest-path tools/asan/Cargo.toml -- --suite native
qualitygate check --worktree --profile full --format json
```

驱动在临时 Ninja 构建树中配置项目，通过 CMake 的编译及链接 launcher 为实际命令追加 ASan 选项。每条成功命令为其输出写入执行凭据；驱动核对已声明目标和测试程序的凭据，再运行测试。构建系统需支持这些 launcher。未声明的第三方库或未触达路径没有对应的覆盖保证。

## 证据和裁决

工具向 stdout 写 diagnostics JSON，向 stderr 写执行日志。完整执行且无发现时退出 0。发生 ASan 或普通测试失败时退出 10，报告保留类别并按失败测试计数。Gate 据此阻断，包括报错位于未变更行的情况。构建失败、缺少插桩凭据、意外的测试清单、跳过、超时或损坏的报告均属于证据缺口。进程截止时间和输出预算由外层 Qualitygate runner 管理。

`checks[].tools` 探测实际版本，并通过 `inputs` 绑定检查器和清单。工具负责读取 CTest 与 ASan 的观测结果，引擎负责验证报告与退出码是否一致、检查是否完整以及最终裁决。Agent 也可以在项目中用 Python 或 Node.js 实现相同接口，以同一组干净、违规及不完整输入验证替换行为。

## 编译缓存与增量选测

冷构建默认关闭缓存。启用 ccache 时设置 `ASAN_USE_CCACHE=1`，将 `CCACHE_DIR` 设到临时工作区之外，并在 CI 恢复该目录。在未安装 ccache 的节点保持默认设置。可用 `ccache -s` 检查命中。每个套件执行均重新链接并运行其测试。

默认策略运行完整的已声明套件。拆分套件时在清单中增加独立条目，为每个套件声明对应的命令及 `when_changed.paths`。将公共头文件、依赖实现、工具或构建配置变化映射到所有受影响的套件；映射不完整时使用完整套件。先用同一组已知发现对照完整和增量执行结果，再将映射纳入正式策略。

## 回归

在本目录执行独立驱动的协议测试，以及真实工具链测试：

```sh
cargo test --locked --manifest-path tools/asan/Cargo.toml
cargo test --locked --manifest-path tools/asan/Cargo.toml --test cmake -- --ignored
```

真实夹具使用隔离 Git 仓库，覆盖干净执行、跨文件释放后读取、普通测试失败以及缺失插桩产物。对项目自己的测试映射，仍需用该项目的独立预期验证。
