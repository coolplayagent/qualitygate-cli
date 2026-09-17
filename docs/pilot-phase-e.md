# 阶段 E：用 owner 签名授权真实试点启动

> 日期：2026-09-16；工程能力已实现；具体生产试点尚未启动。

## 1. 生成待签主题

先按[阶段 D](pilot-phase-d.md)生成 `observations.json`，在写入任何 observation 前运行：

```bash
qualitygate pilot authorization-subject \
  --input observations.json --format json > authorization-subject.json
```

输出包含 DSSE `payload_type` 和精确 `subject`。subject 绑定 repository、pilot ID、plan seal、
owner、reviewer、seal/start/end、任务数与 assignment 数。命令拒绝未封存、计划已变化或已有观察的清单。

## 2. 在外部信任边界签署

由配置的 human owner 创建以下记录，`subject` 使用上一步的原值：

```json
{
  "schema_version": 1,
  "record_id": "qualitygate-pilot-start-2026-09-16",
  "subject": {},
  "authorizer": {"id": "pilot-owner", "kind": "human"},
  "reason": "Authorize the sealed seven-day comparison",
  "issued_at": 1789516800,
  "expires_at": 1790121660
}
```

外部签名工具用 payload type
`application/vnd.qualitygate.pilot-plan-authorization.v1+json` 对该记录的精确 JSON bytes 做
DSSE PAE 和 Ed25519 签名。信任库 key ID、记录 `authorizer.id` 与协议 owner 必须相等；key 需设置
`checks: ["pilot-plan-authorization"]` 和 `allow_repository_checks: true`。`issued_at` 位于 seal/start
之间，`expires_at` 覆盖 end，信任库 `max_age_seconds` 也需覆盖该有效期。

CLI 不创建或保存私钥。信任库与 DSSE envelope 必须放在被测仓库之外，且不能是符号链接文件。

## 3. 验证并汇总

```bash
qualitygate pilot summarize \
  --input observations.json \
  --trust-store /external/pilot/trust.json \
  --authorization /external/pilot/start.dsse.json \
  --format json
```

有效结果包含 `plan_authorization.status=authenticated`、signer key ID、公钥摘要、信任库与授权
envelope 的来源和摘要。
不提供两个外部参数时仍可描述性汇总，但状态为 `absent`，`protocol_ready` 保持 false。只提供其中
一个参数会被 CLI 拒绝。错仓库、错计划、签名篡改、错误 owner/scope、过期、撤销或仓库内信任输入
均作为验证未完成，不会退回成成功的未签名汇总。

## 4. 证据边界

有效签名证明受信 key 授权了精确封存主题，也允许在后续汇总时按最新撤销状态重新验证。
它不提供独立可信时间戳；reviewer、任务真值、成本、归档服务和 observation 内容仍按各自证据审查。
最终试点接受继续需要七天真实观察、32 个运行单元、预定阈值和独立 reviewer 记录。

当前仓库没有具体生产任务、外部 owner/reviewer 和正式证据库，因此本阶段没有生成正式授权、
没有开始七天时钟。集成测试仅使用临时仓库、临时外部目录和固定测试 key。

观察完成后，按[阶段 F](pilot-phase-f.md)生成阈值评估并由独立 reviewer 签署最终决定；启动授权
本身仍不代表接受。

## 5. 需求到测试

| 需求 | 验证 |
|---|---|
| E-01/E-02 | `pilot_authorization::cli_authenticates_the_owner_and_exact_sealed_start_subject` |
| E-03/E-05 | `pilot_authorization::cli_rejects_repository_owned_or_foreign_authorization_inputs` |
| E-03/E-04 | `adapters::pilot_authorization::owner_signature_binds_repository_plan_and_window` |
| E-04 | `adapters::pilot_authorization::authorization_rejects_expiry_revocation_and_nonhuman_owner` |
| E-06 | `domain::pilot::summary_accepts_only_verified_authorization_for_its_exact_plan`，CLI 集成测试比较 authenticated 与 absent 汇总 |

## 6. 工程质量结果

2026-09-16 的本地 Linux 验证通过格式、全部目标/特性编译、Clippy warnings denied 和
全部目标/特性测试：417 项通过，12 项需要外部生产者、真实凭据或收费模型的测试按合同忽略。
LLVM 覆盖率为 95.81% Rust lines（21,834 行，915 行未覆盖），超过 90% 门禁。

full selfcheck 的 338 个 fixture 全部与 golden 一致。架构报告覆盖 157 个源摘要和 1,480 条
文件/行依赖，0 违规。domain、config、adapters、application、interfaces 五个 Bazel 单元目标
通过，根 CLI Bazel target 构建通过。CodeSpec 与 Knowledge map 均有效。日志和机器可读报告
保存在 `target/pilot-phase-e-verification/`。

此前用户要求的本地开发 Skill 也已更新到 `~/.codex/skills/qualitygate-cli`。staged 和安装后
selfcheck 均为 338/338，安装后的 `authorization-subject` 命令可用，release binary 摘要为
`sha256:4b938f82e92dc1a4e898a1b4c69246aeecfc95456350ab78bad4189d8d47fcff`；上一安装备份在
`~/.codex/skill-backups/qualitygate-cli-20260916T063240Z`。这仍是带有 dirty source 标记的本地
development install，不是发布版本。

这些证据只覆盖工程实现。没有运行新的模型调用，没有生成正式 production owner 签名，
没有开始七天观察；Miri、ASan 和外部 producer 继续属于各自独立门禁。
