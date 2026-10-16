# 策略、任务与签名证据

[English](../../en/02-reference/04-policy-task-and-signed-evidence.md) · [本卷目录](README.md)

仓库策略、任务验收和外部信任是三种不同权威。选中策略定义必需检查与严重级别；任务契约为一次请求
增加验收项；trust store 与签名记录认证特定人工或系统声明。三者都不能静默削弱另一方。

## 策略生命周期

候选修订记录父版本、来源输入、不可变工件、验证矩阵和观察差异。候选推广前应在相同 base/head 与
执行许可下完成独立成对验证。批准和回滚使用不同签名主题；active policy 历史可审计且只追加。
本地分支名或摘要本身不代表批准。

相同 base/head 指相同 Git 修订。各策略独立应用自己的 exclude；若执行输入不同，验证证据绑定
两份快照身份，不能共享某一策略裁剪后的内容。
相同 exclude 可以复用不可变树；不同 exclude 在单个用例内顺序执行，保持两棵树的内存预留上限。

受保护套件的可选 `budget.snapshot_max_file_mib` 默认 2，允许整数 1–8；它共同约束基线/候选
采集及测试 overlay。该字段受外部授权的套件摘要约束，因此容量变更需要更新外部授权，不能
通过修改候选仓库策略覆盖。

诊断趋势、oracle 审核、gate 执行、下游收益和维护成本属于不同观察。分母或数据缺失时保持 unknown。
策略演进不能为了改善指标而把 warning 或未完成证据改写为通过。

## 任务计划与复验

任务文件包含稳定 task ID、验收项 ID、可观察行为、验证类型、关联检查、requiredness 与 severity。
报告把任务摘要与策略、快照一起保存。命令式验证保留精确调用，并证明预期测试/报告确实产生。

复验上下文绑定原报告、选择器、策略引用、任务、资源参数和证据输入。最终快照变化后必须生成新报告；
`quick`、路径范围或仍有待执行交付项的结果不能满足 full 验收。

## 签名记录与谱系

人工验收及试点授权/接受使用调用方控制、位于被测仓库外的 DSSE/Ed25519 信任输入。验证检查精确主题
字节、key scope、角色、有效期、最大年龄、撤销，以及与快照/策略/任务的绑定。记录缺失、过期、
来自错误主体、已撤销或格式错误时均为未完成。

`snapshot.content_digest` 标识采集后的执行文件。存在 `snapshot.verification_digest` 时，它还绑定
检查范围、exclude、变更行及路径过滤，签名快照绑定也必须包含该字段。文件内容相同不代表
delivery 与 repository 范围的证据可以互换。
可复用核心的 repository 评估带路径过滤时也必须绑定路径范围；只有没有 exclude 和路径过滤的
repository 评估才可仅使用 content digest。CLI 只验证交付范围，因此核心全仓评估的同范围
CLI 复查 argv 为空；独立的 delivery_recheck 明确用于交付范围验证。
反馈将不可用或空的复查命令保留为 `argv: null`、`omitted: true`，同时保留可执行的 delivery_recheck。

Agent provenance 认证所声明的输入、输出、变换和工具身份；Git trailer 把声明绑定到提交，并拒绝
歧义或缺失关联。这些机制证明谱系，不证明语义正确或独立审核。

版本化 decision/feedback envelope 保留原始确定性 payload，以及证据摘要、warning、gap、pending 与
omission。可选外部 judgment provider 只能在 shadow/advisory 模式评估 warning；概率结果、abstain、
非法输出或执行缺口都不能改变确定性 gate 或批准策略。

---

[上一章：项目与工具报告](03-projects-and-tool-reports.md) · [全书目录](../README.md) · [下一章：退出码与资源限制](05-exit-codes-and-resource-limits.md)

## 默认排除文档与媒体资源

`unstructured_documents` 缺省为 `enabled: true`、`retain: []`，已有配置（包括
`exclude: []`）也生效，因此升级会改变这些策略的采集覆盖范围。新初始化配置会显式
写出默认值。运行时通过 `snapshot.unstructured-documents.v1` 声明支持此能力。

```yaml
unstructured_documents:
  enabled: true
  retain: ['assets/required.pdf', 'tests/fixtures/**']
```

版本 1 分类器按文件名最后一个后缀识别，不区分 ASCII 大小写：

- 文档：`pdf doc docx xls xlsx ppt pptx odt ods odp rtf`。
- 图片：`png jpg jpeg gif webp bmp tif tiff ico avif heic heif`。
- 音视频：`mp3 wav flac aac ogg m4a mp4 mov avi mkv webm mpeg mpg`。

不检查内容，也不根据大小猜测类型。Markdown、纯文本、HTML、SVG、字体、压缩包、
未知 `.bin` 及其他未列出的后缀仍参与采集。因此 9 MiB PDF 可被排除，9 MiB `.bin`
仍导致采集 `incomplete`。本能力不提高单文件 8 MiB、总量 1 GiB 的容量上限。

默认排除发生在读取内容和计算字节预算之前，统一作用于 base/target、工作区、索引、
diff/MR 和历史快照。排除的资源不进入命令执行目录，不接受规则检查，也没有内容完整性
保证。命令依赖的资源须恢复采集；缺少命令证据不能判定通过。仅修改排除资源时，报告
受检交付为空，不声称资源通过检查。

`retain` 复用 `exclude` 的有界、区分大小写、仓库相对路径 glob，仅恢复默认排除。
策略、任务契约、规则来源、验证资产、来源证据和声明的工具输入自动保留。显式
`exclude` 优先于 `retain`，命中受保护输入仍报错。`enabled: false` 仅关闭默认排除。
恢复采集的文件仍受正常读取、类型和容量约束。

`selection.resource_exclusions` 记录分类版本、有效配置、保护路径以及分别统计的
`default_excluded_*`、`explicit_excluded_*` 数量和路径；字段名区分默认及显式排除原因。
验证摘要绑定这些证据与完整排除清单，不哈希被排除的内容。doctor 预算及 init 预检提供
同样的分类证据，但路径详情有上限、总数完整，须查看 `details_truncated`。原有
`excluded_paths` 仍为两类排除的合并清单。
