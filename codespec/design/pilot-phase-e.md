# 阶段 E：认证试点启动授权

> 日期：2026-09-16；状态：工程实现；需求：EVO-05-01、EVO-05-04。

## 交付契约

| 编号 | 交付及反例 |
|---|---|
| E-01 | `pilot authorization-subject --input` 从尚无 observation 的有效封存清单导出唯一签名主题；未封存、已观察或计划摘要失配不产生主题 |
| E-02 | 主题绑定 repository、pilot ID、plan seal、owner、独立 reviewer、seal/start/end、任务数与 assignment 数，不依赖调用方另行拼装摘要 |
| E-03 | `pilot summarize` 只有同时收到仓库外 `--trust-store` 和 `--authorization` 时才尝试认证；信任 key 必须匹配 owner、允许仓库级检查并具备 `pilot-plan-authorization` scope |
| E-04 | DSSE/Ed25519 记录必须由 human owner 签署，签发时间位于 seal 与 start 之间，有效期覆盖完整观察窗口，并通过当前时间、最大年龄与撤销检查 |
| E-05 | 信任库和授权记录有大小、类型、外部路径及读取后不变约束；错仓库、错主题、弱/重复 key、篡改、过期、撤销和读取期间变化均不降级为未签名成功 |
| E-06 | 汇总区分 `authenticated` 与 `absent`，缺少授权使 `protocol_ready=false`；授权只认证启动主体，不提供独立可信时间，也不批准最终试点或推广 |

## 数据与所有权

`domain::pilot` 定义签名主题、记录和已验证结果，并在纯汇总中把认证状态纳入协议限制。
`adapters::pilot_authorization` 复用既有 DSSE PAE、Ed25519 严格验证和 trust-store 有效期逻辑；
它只解释已经认证的 payload。`application::external` 有界读取仓库外输入并在发布前重读，
`application::pilot` 编排主题生成、认证与汇总，`interfaces` 只解析参数和渲染结果。

CLI 不生成私钥、不选择 owner，也不签署记录。信任库仍由调用方控制，但它必须位于被测仓库之外；
因此仓库代码不能通过提交自己的 key 获得授权。owner 字符串必须等于 trust-store key ID，
reviewer 继续作为被签主题中的独立身份，最终 reviewer 接受记录仍属于外部流程。

## 时间与接受边界

授权 payload 中的 `issued_at` 必须在封存后、观察开始前，`expires_at` 必须覆盖观察结束；
验证还使用当前系统时间、信任库最大年龄和撤销清单。这些检查阻止明显的未来、过期和短期授权，
但普通签名不等于可信时间戳，签名者仍可能声明一个历史时间。持久归档或时间戳服务由团队选择。

阶段 E 解决阶段 D 明示的调用者身份缺口。它不补造具体 8 个生产任务，不启动七天计时，
也不把完整汇总变成最终接受。真实观察结束后仍需独立 reviewer 根据预定阈值签署外部验收记录。

## 验证

adapter 单元测试覆盖有效 owner 签名、计划摘要漂移、晚签、窗口不足、撤销和 Agent 自签。
`pilot_authorization` 集成测试覆盖主题导出、仓库外 trust/record、认证汇总、缺失授权状态和
仓库内 trust 拒绝。`pilot_seal`、`pilot_summary` 与纯 metric 测试继续覆盖前后阶段边界。
