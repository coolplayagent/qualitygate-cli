# IG-19 · 对等节点扩容与资源配额实验

AWS 在 2020-11-25 的 Kinesis 事故复盘中说明，前端为每个其他前端创建 OS 线程。新增节点让所有前端同时消耗更多线程，超过 OS 配置的上限后，前端无法构造可用于路由的分片映射。冷启动重建映射也与正在处理的请求争用资源。下面的模型把这条故障链转成可单独验证的本地容量和发布约束。[AWS 复盘](https://aws.amazon.com/message/11201/) · [防护规格](../requirements/production-incident-guards.md)

## 断言与结果

[模型](../../fixtures/production-incident-guards/capacity.rs)把一个前端正在使用的两条旧路由、三条新路由的临时槽和服务保留的两个槽相加，得到切换期间所需配额 7。每个对等节点占一个模拟线程槽，缺少路由元数据在其槽位分配之后才被发现。修复版本先检查配额，构建完整后才切换，失败时归还临时槽。两种版本共用以下六条断言。

| 场景 | 断言 | 缺陷路径 | 修复路径 |
| --- | --- | --- | --- |
| 配额为 7 | 峰值占用 5，留 2 个服务槽，切换后可读取新路由，旧槽退还 | 通过 | 通过 |
| 配额为 6 | 在分配前拒绝，旧路由继续可读，服务留有余量 | 失败 | 通过 |
| 配额为 4 | 在耗尽槽位前拒绝，保留完整旧路由 | 失败 | 通过 |
| 配额为 7，三次元数据不完整 | 每次暂用两槽后归还，期间与之后的资源占用保持在对应预算内，旧路由仍可读 | 失败 | 通过 |
| 元数据恢复 | 重试成功，读到完整的新路由，旧槽退还 | 失败 | 通过 |
| 配额从 6 升至 7 | 第一次拒绝，第二次成功并继续服务 | 失败 | 通过 |

2026-11-18 在 Linux x86_64 的隔离 Git 仓库运行。Qualitygate 1.3.0 的二进制 SHA-256 为 `c8e402349a10acf4eb0610912c921d51890e9dcbb6ecda88f161188888913da0`，编译使用 rustc 1.97.1，Git 版本为 2.53.0。三轮的 `profile` 均为 `full`，`scope` 均为 `delivery`。正常执行的检查以退出 0 和 JUnit 报告传回六条结果。

| 运行 | JUnit 失败数 / 执行数 | Gate 结论 | JUnit SHA-256 |
| --- | --- | --- | --- |
| 缺陷路径 | 5 / 6 | complete=true，fail，退出 1 | `9e59f4d39f4bcc7c540d3352d669163a3fcfe83be197d9c5a90cd58e4960305b` |
| 修复路径 | 0 / 6 | complete=true，pass，退出 0 | `b645e9b17b6992897b02658265cb1c6aaa0c391615b6953874c73052db945f19` |
| 仅运行版本探针 | 未生成要求的报告 | complete=false，incomplete，退出 2，检查为 tool_error | — |

此证据覆盖单前端、串行刷新和六个有限输入。实际系统的线程、栈、连接及内存分别计量。冷启动时没有上一份可用映射，恢复演练还要验证流量恢复速率、调度与并发、节点规格和配额变化。

## 复查

从项目根目录运行以下 Bash 片段，令 `QG` 指向可信的 Qualitygate 可执行文件绝对路径。需要 Git 和 PATH 中的 rustc。

```bash
set -eu
: "${QG:?请设置 Qualitygate 可执行文件的绝对路径}"
case_dir=$(mktemp -d)
repo="$case_dir/repo"
git init -q "$repo"
git -C "$repo" -c user.name=Fixture -c user.email=fixture@example.invalid commit --allow-empty -qm baseline
cp fixtures/production-incident-guards/capacity.rs "$repo/capacity.rs"
printf 'target/\ncapacity-check\n' > "$repo/.gitignore"

write_policy() {
    cat > "$repo/qualitygate.yaml" <<YAML
schema_version: 1
rulesets: []
checks:
  - id: compile-capacity
    argv: [rustc, --edition=2024, -Dwarnings, capacity.rs, -o, capacity-check]
    tools: [{id: rustc, argv: [rustc, --version], inputs: [capacity.rs]}]
    timeout_seconds: 30
  - id: capacity
    argv: [./capacity-check, "$1"]
    depends_on: [compile-capacity]
    tools: [{id: capacity-check, argv: [./capacity-check, --version], inputs: [capacity.rs]}]
    reports: [{path: target/capacity.xml, format: junit, mode: full, minimum_tests: 6}]
    timeout_seconds: 5
YAML
}
check_case() {
    local label=$1 expected=$2 actual=0
    write_policy "$3"
    "$QG" --root "$repo" check --worktree --profile full --format json \
        --output-dir "$case_dir/$label" > "$case_dir/$label.json" || actual=$?
    printf '%s exit=%s report=%s/%s.json\n' "$label" "$actual" "$case_dir" "$label"
    test "$actual" -eq "$expected"
}
check_case before 1 before
check_case after 0 after
check_case missing 2 --version
```
