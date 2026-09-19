# Rust 阶段 A 任务模板 v1

这六个模板使用当前任务契约，不需要新增检查命令。
适用于已具备锁文件、可离线构建和独立命名集成测试的 Cargo 项目。
实际采用前必须检查测试是否表达当前任务；工作区成员、特性、依赖安装、额外回归与接口验证由团队补齐。

- [bug-fix.yaml](bug-fix.yaml)：为已审查的缺陷回归记录旧代码失败和修复后通过。
- [refactor.yaml](refactor.yaml)：对改动前后运行相同的行为回归，允许两侧通过。
- [feature.yaml](feature.yaml)：验证新功能场景及相关既有行为。
- [dependency.yaml](dependency.yaml)：验证选定 lockfile、特性和兼容性场景。
- [documentation.yaml](documentation.yaml)：验证文档或规则示例与仓库行为一致。
- [performance.yaml](performance.yaml)：先验证功能，再要求外部签名的同环境配对测量复核。

文件名与任务 ID 中的 `v1` 标识模板版本；每个真实任务采用时应指定自己的任务 ID 和可观察描述，
并记录模板版本与最终契约摘要。将任务文件纳入选定策略的 Git 提交，使用已有 `check --task` 执行。
模板不会自动启用或修改仓库策略，不能因为被复制到候选工作区就获得信任。

Rust/Cargo 版本探测和锁文件是必需输入，实际测试数不得为零。
模板使用 CLI 已支持的 Cargo 测试汇总；不声明只能与结构化报告合用的 `findings_exit_codes`。
已完成但零测试的命令产生阻塞违规；编译失败没有有效测试汇总，验证未完成。
Cargo 的退出码 101 也可能来自编译错误，只有实际执行报告中的断言失败才能作为缺陷反例；
调用方必须保留完整执行状态。模板本身只检查当前一次执行，不自动判定两份报告的行为等价或反例有效性。
逐新增/修改测试文件的反例证明需另行配置 [test_effectiveness](../../../docs/en/02-reference/02-rules.md)。

阶段范围及真实收益边界见[执行设计](../../../codespec/design/pilot-phase-a.md)和
[试点准备记录](../../../docs/en/01-user-guide/05-real-repository-pilot.md)。
阶段 C 的生产者限制、汇总口径和反例见[证据记录](../../../docs/en/01-user-guide/05-real-repository-pilot.md)。
具体任务采用后，应先按[阶段 D](../../../docs/en/01-user-guide/05-real-repository-pilot.md)组成完整模型×流程 assignment
矩阵并执行 `pilot seal`，再开始写入 observation。
新试点还应按[阶段 E](../../../docs/en/01-user-guide/05-real-repository-pilot.md)导出精确主题，并由仓库外受信 human owner
签署启动授权；模板和 Agent 不能自行创建信任 key。
观察完成后按[阶段 F](../../../docs/en/01-user-guide/05-real-repository-pilot.md)生成验收主题，由独立 reviewer 签署接受或拒绝。
新试点使用[观察清单 v6](../observation-v6.json)，按[阶段 G](../../../docs/en/01-user-guide/05-real-repository-pilot.md)
在封存前填写结构化总预算，并按[阶段 H](../../../docs/en/01-user-guide/05-real-repository-pilot.md)核对
4 个缺陷修复/4 个重构的独立任务配额，再按[阶段 I](../../../docs/en/01-user-guide/05-real-repository-pilot.md)
提供每项任务的来源工件，再按[阶段 J](../../../docs/en/01-user-guide/05-real-repository-pilot.md)封存交替执行顺序并
记录实际启动位置，最后按[阶段 K](../../../docs/en/01-user-guide/05-real-repository-pilot.md)提供每运行单元的
初始完整报告和无进展停止参数；旧 v1–v5 清单维持原摘要和验收解释。
