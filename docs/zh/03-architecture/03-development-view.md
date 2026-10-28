# 开发视图

[English](../../en/03-architecture/03-development-view.md) · [本卷目录](README.md)

开发视图把逻辑职责映射到 Rust 源码 owner、允许 import、测试面和构建期约束。它回答“修改应放在哪里、
允许依赖什么”。

Rust crate 使用显式顶层 owner 和无环依赖图。crate root 只声明 owner；可执行入口仅委托给接口层。

```mermaid
flowchart TD
    main --> interfaces
    interfaces --> application
    interfaces --> config
    interfaces --> snapshot
    application --> adapters
    application --> config
    application --> snapshot
    application --> runner
    application --> net
    adapters --> config
    adapters --> snapshot
    snapshot --> net
    snapshot --> paths
    snapshot --> runner
    config --> paths
    paths --> env
    net --> env
    runner --> env
    interfaces --> domain
    application --> domain
    adapters --> domain
    config --> domain
    snapshot --> domain
    net --> domain
    runner --> domain

    classDef pure fill:#eef7ee,stroke:#2f6f3e,color:#183d22
    classDef boundary fill:#fff4df,stroke:#9a6700,color:#4d3500
    classDef orchestration fill:#eaf2ff,stroke:#315f9b,color:#17365d
    class domain pure
    class env,paths,net,runner,snapshot boundary
    class application,interfaces,main orchestration
```

箭头表示“允许依赖”。所有 owner 都可使用纯 domain 契约，但只有命名的边界 owner 才能访问相应外部
能力。依赖图有意保持非对称：adapter 可消费 snapshot 事实，snapshot 不能反向导入 adapter；
interfaces 调用 application 用例，application 不解析 CLI 参数。

| Owner | 职责 |
| --- | --- |
| `domain` | 可序列化契约与纯 gate 决策；禁止进程、文件系统、网络和环境 I/O。 |
| `env` | 环境变量读取及其派生值。 |
| `paths` | 受限路径验证与安全解析。 |
| `net` | 有界网络访问和远端获取契约。 |
| `runner` | 有界异步进程执行与证据捕获。 |
| `snapshot` | Git 选择器、不可变文件获取、变更映射与物化。 |
| `config` | 严格的策略/任务加载、验证与计划。 |
| `adapters` | 语言/项目事实与外部报告归一化。 |
| `application` | 检查编排、反馈、复验和用例协调。 |
| `interfaces` | CLI 参数解析与输出渲染。 |
| `main` | 只委托给 `interfaces` 的进程入口。 |

共享契约应放在 `domain`，而不是 CLI 或具体适配器中。接口层不直接执行外部 I/O。阻塞的文件系统与
解析工作和异步编排隔离；网络、环境和路径访问各有专门 owner，便于能力审核。

`domain::xml_budget` 负责配置发现与适配器共享的纯命名空间工作量计数，只接收计数、限制作用域
栈并检查算术。XML token 化、树解析和各 profile 的错误语义仍归各自的消费层负责。

`config::yaml_budget` 统一负责 YAML 展开预检，在既有重复键和类型化解码前访问值，
不物化集合，也不复制别名字符串。配置、任务、规则和验证套件共享该边界；application
的策略信任加载在既有阻塞工作中调用配置解码器。

`adapters` 内的 `xml_document` 统一负责报告、Maven 模型和兼容性证据的有界 XML 树解析。
调用方执行各自的字节预算，报告特有的 DOCTYPE 归一化留在 `reports`。迭代预检在树解析器比较
重复名称之前，限制每个元素 1,024 个属性、整个文档 100,000 个属性，命名空间声明计入。
项目和兼容性适配器
依赖这个共享叶模块，无需导入报告格式处理代码。

配置发现的建议性 Maven 清单校验由 `config::discovery::maven_manifest` 负责，在生成命令
候选前限制 XML 深度、节点和属性数量，并把不支持的清单记录为发现缺口。该 profile 位于 `config` 内，
不导入适配器，也不产生已解析的项目事实；适配器报告失败继续使用独立的执行状态语义。

架构测试解析全部生产 Rust 源码，解析模块与 import 路径（含分组、别名、宏和条件引用），检查 owner
边、禁止能力和环，并输出可审核的源码摘要/行号报告及 DOT 图。隐藏根导出、歧义模块文件、未知 owner、
宽泛外部 glob 和纯 domain 中的 I/O 都会被拒绝。

文档、示例、测试与构建元数据可调用公共接口，但不会形成新的生产 owner。修改模块所有权或 import 后，
除编译外还必须运行架构门禁并审核其边证据。

这种划分让审核者分别回答三个问题：决策是否纯且可序列化，外部动作是否受限有界，以及编排是否在不
产生依赖环的前提下组合能力；同时也让可复用 library 不依赖可执行接口。

---

[上一章：进程视图](02-process-view.md) · [全书目录](../README.md) · [下一章：物理视图](04-physical-view.md)
