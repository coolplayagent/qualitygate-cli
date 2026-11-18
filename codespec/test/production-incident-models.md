# 事故防护模型的 CLI 实测

2026-11-18 在 Linux x86_64 的隔离 Git 仓库运行。两个模型对应[事故约束 IG-01 和 IG-04](../requirements/production-incident-guards.md)，使用仓库开发版 Qualitygate 1.3.0、rustc 1.97.1、Git 2.53.0。CLI 可执行文件 SHA-256 为 `e4a3097455b33f0d9e0135fae519d2da7d079b2d6bc2d6eac10ac7613e4c6696`。每个快照先编译同一套 [Rust 检查器](../../fixtures/production-incident-guards/model_check.rs) 和所选的被测实现，再让两个独立检查各写一份 JUnit 报告。[策略](../../fixtures/production-incident-guards/policy.yaml)声明了版本探针、源码输入、超时和最低测试数。

## 断言与反例

- **IG-04，生成配置的唯一性和容量。** 模型把容量缩为 2，最近一次有效的列表为 `[9]`。发布 `[4]` 和 `[4,5]` 应当更新活动配置；发布 `[4,4]` 和 `[4,5,6]` 应当保留 `[9]`。缺陷版直接替换活动配置；修复版写入前校验唯一性和容量。生产数据库查询、跨库字段来源和代理读取路径另需端到端覆盖。
- **IG-01，过期提交与清理。** 初始活动代为 0，0、1、2 代均有数据。旧计划目标为 1，新计划目标为 2。两个计划各有准备和提交两步，收集器可执行一次并删除早于活动代的数据。每次提交须晚于对应准备，五步共有 30 种合法排列。每一步后检查活动代单调且数据仍在，结束时应为 2。缺陷版提交沿用准备时的检查。在 `ONnCo` 顺序中，旧、新计划依次准备，新计划提交，收集器清理，最后旧计划提交，活动代退回到已被清理的 1。修复版在提交时再次检查活动代和数据。模型给每个步骤赋予原子语义，生产实现仍需确认实际事务边界及其一致性。

[缺陷版本](../../fixtures/production-incident-guards/subject_before.rs)和[修复版本](../../fixtures/production-incident-guards/subject_after.rs)共享同一组预期、枚举器和门禁配置。超时、报告缺失两轮只在临时仓库替换生产者命令。

## 观测结果

下表以“失败数 / 执行数”列示各套件。前五轮均为 `scope: delivery`、`profile: full`、`policy.trust: local_candidate`，待交付检查列表为空。两套 JUnit 的生产者均将正常完成编码为命令成功，门禁读取实际用例的失败记录。

| 输入与故障条件 | configuration | schedules | 整体结果 |
| --- | --- | --- | --- |
| 暂存缺陷版本 | 2 / 4，duplicate、over-limit | 10 / 30 | complete=true，fail，退出 1 |
| 工作树修复，暂存区仍为缺陷版本 | 0 / 4 | 0 / 30 | complete=true，pass，退出 0 |
| 工作树修复后重查暂存区 | 2 / 4 | 10 / 30 | complete=true，fail，退出 1 |
| 修复版本，配置生产者成功退出但未生成报告 | tool_error，原因含 `Required report was not produced`，无 verdict | 0 / 30 | complete=false，incomplete，退出 2 |
| 修复版本，配置生产者等待 10 秒，时限 1 秒 | timed_out，无 verdict | 0 / 30 | complete=false，incomplete，退出 2 |
| 提交暂存版本后，要求工作树仍基于前一提交 | 未运行 | 未运行 | complete=false，incomplete，退出 2，`verified_shapes` 为空 |

最后一轮阻塞原因为 `Snapshot base differs from the expected base; select a new comparison explicitly`。两次暂存区运行的快照内容 SHA-256 同为 `36c4e5ae892d2942b7b348a52a70d4963e557e486e02522c2131c5fc0130ba62`，工作树修复后的内容摘要为 `b85ee9f479aa7e16fd665677382fb6fd5aec210c58eb74f62c32d96fc04bf4e5`。

| 工件 | 缺陷版本 JUnit SHA-256 | 修复版本 JUnit SHA-256 |
| --- | --- | --- |
| configuration | `ecb6c481a95a4491c1f03087a0825049a0101000472696c19dcbb0ce8993f34b` | `fe8fa1dba8b5cd64fd44b98cc2c920b70a018e6b8cf8d01386eaf4f20a26cd0c` |
| schedules | `4b6f8708773aaeb02472e0d02c275588c48c94711ae431869e8877731eca8e4d` | `d201f1ccce440319dd0fcefe052fcbf404738b21cfb88698137a130407ab57e1` |

修复轮的 `verification.conclusion` 为“在已验证形态下未发现问题”。该结论覆盖所选快照上 4 个配置输入和 30 个调度顺序。网络延迟分布、多个节点真实存储的一致性、故障域和现网命中率仍需另外取证。

## 复查

从仓库根目录在 Linux 和 Bash 下运行。需要本仓库开发 CLI、匹配的规则资产、PATH 中可执行的 rustc、Git 和 coreutils。下列操作只写临时仓库和同级报告目录。每次 `check` 都会重新采集输入并编译所选版本。

```bash
set -eu
qg="$PWD/target/debug/qualitygate"
model_src="$PWD/fixtures/production-incident-guards"
export QUALITYGATE_BUILTIN_RULES_DIR="$PWD/skills/qualitygate-cli/references/rules"
case_dir=$(mktemp -d)
repo="$case_dir/repo"
git init -q "$repo"
git -C "$repo" -c user.name=Fixture -c user.email=fixture@example.invalid commit --allow-empty -qm baseline
cp "$model_src/model_check.rs" "$repo/model_check.rs"
cp "$model_src/subject_before.rs" "$repo/subject.rs"
cp "$model_src/policy.yaml" "$repo/qualitygate.yaml"
printf 'target/\nmodel-check\n' > "$repo/.gitignore"
git -C "$repo" add .

check_model() {
    local label=$1 expected=$2 actual=0
    shift 2
    "$qg" --root "$repo" check "$@" --profile full --format json \
        --output-dir "$case_dir/$label" > "$case_dir/$label.json" || actual=$?
    printf '%s exit=%s report=%s/%s.json\n' "$label" "$actual" "$case_dir" "$label"
    test "$actual" -eq "$expected"
}

check_model before 1 --staged
cp "$model_src/subject_after.rs" "$repo/subject.rs"
check_model after 0 --worktree
check_model replay-index 1 --staged

sed 's/argv: \[\.\/model-check, configuration\]/argv: [git, --version]/' \
    "$model_src/policy.yaml" > "$repo/qualitygate.yaml"
check_model missing 2 --worktree

sed -e 's/argv: \[\.\/model-check, configuration\]/argv: [sleep, "10"]/' \
    -e 's/timeout_seconds: 5/timeout_seconds: 1/g' \
    "$model_src/policy.yaml" > "$repo/qualitygate.yaml"
check_model timeout 2 --worktree

cp "$model_src/policy.yaml" "$repo/qualitygate.yaml"
git -C "$repo" -c user.name=Fixture -c user.email=fixture@example.invalid commit -qm before
check_model wrong-base 2 --worktree --expect-base "$(git -C "$repo" rev-parse HEAD^)"
```

逐轮报告可检查 `gate`、`checks[].execution.status`、`checks[].verdict`、`checks[].metadata.tests` 和 `verification`。完整执行的原始 JUnit 路径及摘要在对应检查的 `execution.artifacts` 中。

## IG-17 · 编译器验证同一项目中的 Java 模块环

在一个临时项目中定义三个 JPMS 命名模块。每条边都在调用方的 `module-info.java` 中声明 `requires`，其 Java 源码同时引用对方导出的 `Entry` 类。用已安装的 `javac 25.0.1` 编译三个模块，Qualitygate 通过固定命令和版本探针采集结果。

| 模块图或执行条件 | 编译器证据 | Qualitygate 的 full / delivery 结果 |
| --- | --- | --- |
| alpha → beta → alpha，gamma 独立 | 退出 1，两个描述符均给出 `cyclic dependence` | completed / fail，完整，退出 1 |
| alpha → beta → gamma → alpha | 退出 1，三个描述符均给出 `cyclic dependence` | completed / fail，完整，退出 1 |
| alpha → beta → gamma | 退出 0，标准错误为空 | completed / pass，完整，退出 0 |
| 同一无环图，策略指向不存在的编译器 | 找不到可执行文件，编译未启动 | tool_error，无 verdict，不完整，退出 2 |

四轮的 `plan.pending_delivery_checks` 均为空。两个失败的原因可在保留的编译器 stderr 核对；其他编译错误也会使这项命令检查失败。该证据覆盖编译器本次解析的 JPMS 模块描述符。单个 JPMS 模块内部的类或包依赖，以及运行时服务间的调用图，需要相应粒度的检查器。

**复查。** 接上节的 Bash 环境，保留 `qg`、`check_model` 和规则资产路径，并把 JDK 的 `bin` 放入 PATH。另建临时仓库，在四轮报告中核对 `checks[0].execution` 和 `gate`。

```bash
case_dir=$(mktemp -d)
repo="$case_dir/repo"
git init -q "$repo"
git -C "$repo" -c user.name=Fixture -c user.email=fixture@example.invalid commit --allow-empty -qm base
printf 'target/\n' > "$repo/.gitignore"
cat > "$repo/qualitygate.yaml" <<'YAML'
schema_version: 1
rulesets: []
checks:
  - id: jpms-compile
    argv: [javac, --module-source-path, src, -d, target, --module, "alpha,beta,gamma"]
    tools: [{id: javac, argv: [javac, --version]}]
    timeout_seconds: 30
YAML
module_src() {
    local id=$1 next=${2-} requires='' dependency=String
    mkdir -p "$repo/src/$id/$id"
    if [ -n "$next" ]; then
        requires="requires $next;"
        dependency="$next.Entry"
    fi
    printf 'module %s { exports %s; %s }\n' "$id" "$id" "$requires" > "$repo/src/$id/module-info.java"
    printf 'package %s; public class Entry { public Class<?> target() { return %s.class; } }\n' \
        "$id" "$dependency" > "$repo/src/$id/$id/Entry.java"
}
module_src alpha beta
module_src beta alpha
module_src gamma
check_model two 1 --worktree
module_src beta gamma
module_src gamma alpha
check_model three 1 --worktree
module_src gamma
check_model dag 0 --worktree
sed -i 's/\[javac,/[unset-fixture-javac,/g' "$repo/qualitygate.yaml"
check_model tool-missing 2 --worktree
```
