# 阶段 C：依证据演进与试点汇总

> 日期：2026-09-16；需求：EVO-04、EVO-05；业务试点仍待验收。

## 1. 案例来源与规则演进

证据记录可声明案例 ID、生成或实际观察来源、生成/人工确认/独立分类、人工复核者、
是否用于调参、派生证据、预期拦截行为和预期正常行为。声明不认证事实；不可变 archive、
来源摘要和受保护 suite 只保证复查时仍是同一份声明与输入。

受保护 validation suite v2 要求每个案例引用已归档来源。held-out 和 anchor 必须是未由
生成案例派生、未用于调参的独立案例，并由候选作者之外的人类复核。相同案例 ID、来源、
生成/调参历史的重新标记，以及把候选动机同时用作独立验证都会被拒绝。validation、
approval subject 和 promotion 都重新验证这些约束，因此验证之后新增的调参历史会使旧评估失效。
suite v1 保持兼容，但不获得 v2 的独立来源保证。

现有 baseline/candidate 成对执行、受保护 oracle、签名批准、推广、回退和生命周期历史继续
负责策略状态。新增元数据没有自动批准、降级或绕过入口。

## 2. 试点观察清单与命令

复制 [观察清单模板](../templates/pilot/observation-v1.json)，在结果产生前填写协议和全部
assignments。每个 assignment 固定输入 ID、任务契约、base、初始快照、策略、必需检查、
模型请求/实际标识、harness、环境、工具、缓存、权限和预算。报告路径相对清单目录：

```bash
qualitygate pilot summarize --input observations.json --format json
```

CLI 只读加载至多 1 MiB 的严格 JSON 清单和 64 MiB 报告集合，拒绝符号链接、路径逃逸、
长度/摘要变化与重复报告。完整报告必须绑定声明的 base、快照、配置、任务、环境、工具、
profile 和必需检查清单。输出为 `descriptive_only`；它计算指标和阈值观察，不签发试点验收。
清单中的复核者、金额、计价来源和封存时间仍是调用方声明，需由团队证据库及授权记录认证。

分母来自预先声明的 assignments。未执行、失败、超时、放弃、排除和缺报告都保留；任何
证据缺口不会因后续一次成功而消失。末次 full 且实际改变修复快照、在次数/时间预算内、
全部必需检查通过才计修复成功；首次成功、预算内至少一次成功和重复运行全部成功分别报告。
诊断按输入和 canonical diagnostic 去重，确认问题按 canonical issue 去重；未复核诊断使检测率
保持未知。成本包含失败尝试并要求完整币种、计价时间、来源、模型和基础设施部分；token 用量
单独报告，不能代替金额。不同模型、权限、工具、环境、缓存、来源、任务类型和流程分别分组。

只有相同输入集合和声明条件的 `existing_tools` / `qualitygate` 组才计算人工时间、full p95
和每个合格交付成本的描述性比率。模型真实版本未知或没有基线组时不作因果收益结论。

## 3. 阶段 B 记录的离线重放

`examples/pilot_replay.rs` 对已保留的阶段 B index、ledger 及全部引用 artifact 进行摘要/长度
复核，再生成同一观察协议；它不调用模型或重新执行检查。当前离线结果位于
`target/pilot-phase-c/replay`：

| 观察 | 结果 |
|---|---|
| 运行单元 / 模型调用 | 8 / 12 |
| read-only 提案与受限替换 | 4/4 末次 full 通过 |
| workspace-write 兼容性失败组 | 0/4；两次尝试均保留 |
| provider input / output / cached input tokens | 1,616,650 / 17,381 / 1,328,384 |
| 金额、人工时间、实际模型身份 | 未知 |

清单摘要为 `sha256:4030ca92784c80e0579ff466b47904c6f15c754369e1835748e3f8185f4e3685`，
汇总摘要为 `sha256:8a9061f16d4ef1ddf4245cd1179e754e9dd13f677aff0fedd141574ee12e6fb0`。
这些是提取模块的回顾性工程样本，不是预先封存的 8 个生产任务，也没有既有工具配对基线。

## 4. 任务模板与需求到测试

Rust v1 模板补充 feature、dependency、documentation 和 performance。前三类执行独立命名的
真实测试 target；performance 先执行功能契约，再要求外部签名的配对测量复核。模板不创建
基准事实，使用者仍须填写任务特有行为和生产者。

| 需求 | 验证 |
|---|---|
| EVO-04-01/02 | `case_provenance`：生成/调参历史、派生谱系、重复来源、自复核、动机复用和事后污染 |
| EVO-04-03/04 | `case_provenance`：生成 replay 通过但独立正常样例误拦截、已知缺陷漏检、推广前重新验证；既有 policy suites 保留批准/生命周期 |
| EVO-05-01–04 | `domain::pilot` 单元：固定协议、分组、失败分母、重复诊断、未知模型/成本、零分母和不匹配比较 |
| EVO-05 指标与输入完整性 | `pilot_summary`：真实报告绑定、摘要/大小/路径预算、缺失 assignment；`pilot_templates`：正反例、零测试和性能人工证据缺口 |

工程退出还要求格式、编译、Clippy、全目标测试、至少 90% Rust 行覆盖率、full selfcheck、
架构门禁和 CodeSpec/Knowledge map 校验。真实阶段 C 退出仍需要：封存 8 个生产任务和复核人、
完成七天观察、独立案例成对验证、授权推广及可追踪的采用后证据。

## 5. 工程质量结果

2026-09-16 的本地 Linux 工作区验证通过格式、全部目标/特性编译、Clippy warnings denied、
406 项测试；12 项需要真实外部生产者或收费模型的测试按已有约定忽略。LLVM 覆盖率为
95.81% Rust lines（21,374 行，896 行未覆盖），超过 90% 门禁。full selfcheck 的 338 个
fixture 全部与 golden 一致。架构报告覆盖 156 个源摘要和 1,450 条文件/行依赖，0 违规；
受影响的 domain、config、application、interfaces 和根 Bazel contract 共 5 个 target 通过。
CodeSpec 与 Knowledge map 均通过校验。日志保存在 `target/pilot-phase-c-verification`。

这些数字证明当前工作区的工程契约，不证明跨平台运行、被忽略的外部生产者、真实业务收益或
推广授权。Miri 与 ASan 仍属于独立深度门禁，本阶段没有用普通 stable 结果替代它们。
