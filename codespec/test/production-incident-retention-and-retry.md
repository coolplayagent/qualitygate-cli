# 可达对象滞留与多层重试的门禁实验

[IG-18 和 IG-20](../requirements/production-incident-guards.md)共用一份[合成 Rust 负载](../../fixtures/production-incident-guards/retention_retry.rs)。在同一项目策略下运行缺陷和修复两种快照。两项检查均要求完整执行指定用例数，原始观测写入 stderr，diagnostics JSON 给出执行与失败计数。

## 断言

- **IG-18。** 同一进程连续处理 8 批任务，每批分别走完成、取消和到期路径。观察者用 Weak 核对任务进入队列时有一个强引用，并在终态后检查强引用归零。缺陷版在取消与到期后把载荷留在进程存活对象中，即使待处理队列已经空了。修复版只保留终态元数据。此实验直接观测对象寿命和载荷字节，样本总量有上限。它没有估计真实服务堆大小、分配器行为或垃圾回收压力。
- **IG-20。** 由本地脚本返回成功、暂态失败后成功、持续暂态失败、容量拒绝和永久失败五种序列。缺陷版的两层各应用独立的三次尝试额度，还会把容量拒绝重新解释为可重试。修复版由一个入口统一控制实际调用次数，并保留容量拒绝语义。观察者同时断言调用次数和返回结果，防止通过一律拒绝请求绕过检查。本实验使用固定输入序列，退避、抖动、跨进程截止时间和真实过载恢复须另行观测。

## 观测结果

2026-11-18 在 Linux x86_64、rustc 1.97.1 和 Qualitygate 1.3.0 上复跑上节命令。CLI SHA-256 为 c8e402349a10acf4eb0610912c921d51890e9dcbb6ecda88f161188888913da0，探针 SHA-256 为 10e39369ce55bca67fedade4285ec59ad66dc451f17903c8527b49b671195ed4。三轮均使用 full/delivery，待交付检查列表为空。

| 快照 | IG-18 终态记录 / 挂起 / 仍存活载荷 | IG-20 持续暂态 / 容量拒绝的调用数 | 门禁 |
| --- | --- | --- | --- |
| faulty | 24 / 0 / 16，共 4096 字节，失败 16/24 | 9 / 9，容量拒绝变为暂态失败，共失败 2/5 | complete=true，fail，退出 1 |
| fixed | 24 / 0 / 0，失败 0/24 | 3 / 1，正确返回容量拒绝，失败 0/5 | complete=true，pass，退出 0 |
| unknown | 输入版本无法识别，缺少必需报告 | 输入版本无法识别，缺少必需报告 | 两项 tool_error，complete=false，incomplete，退出 2 |

成功、暂态后成功和永久失败对照分别调用后端 1、2、1 次，两种快照均通过。未知版本的执行日志记录 invalid experiment variant，Gate 没有用缺失报告作出测试失败判定。

| 运行 | 快照 content_digest（SHA-256） | CLI stdout JSON 的 SHA-256 |
| --- | --- | --- |
| faulty | c9e45789494c0a6507e0a65734010c2faa9dc8c5d05b9012159eb5f09c328ea0 | 08bcf20beacd729850811d2814e0f0f7f4c51373fe1caefd3ba8c96924d4b512 |
| fixed | 678c806e0e330000b571585d80444dfee5aae79e408f30d5af8b668175225522 | 4cdcb898281679fe7a501322566e99f3cf39d2b240a97c96d71bc5ff6be93af6 |
| unknown | de9fc1a5d177042452cffbbf539b4708eccea93af687c383c76ca9c7293d96e1 | ef3ccb953a30de663ed721d26bf2561fd1c75123c9e605ed2e0b5e9bc1f70524 |

## 复查

从仓库根目录运行。将 QG 设置为选定的 Qualitygate 可执行文件绝对路径，PATH 中应可访问 rustc 和 Git。

```bash
set -eu
: "${QG:?Set the absolute path to Qualitygate}"
case_dir=$(mktemp -d)
repo="$case_dir/repo"
git init -q "$repo"
git -C "$repo" -c user.name=Fixture -c user.email=fixture@example.invalid commit --allow-empty -qm base
cp fixtures/production-incident-guards/retention_retry.rs "$repo/"
rustc --edition=2024 -D warnings "$repo/retention_retry.rs" -o "$case_dir/retention-retry"
cat > "$repo/qualitygate.yaml" <<YAML
schema_version: 1
rulesets: []
checks:
  - id: retention
    argv: ["$case_dir/retention-retry", retention]
    tools: [{id: fixture, argv: ["$case_dir/retention-retry", "--version"], inputs: [retention_retry.rs]}]
    reports: [{path: retention.json, format: diagnostics, from_stdout: true, minimum_tests: 24}]
    timeout_seconds: 15
  - id: retry
    argv: ["$case_dir/retention-retry", retry]
    tools: [{id: fixture, argv: ["$case_dir/retention-retry", "--version"], inputs: [retention_retry.rs]}]
    reports: [{path: retry.json, format: diagnostics, from_stdout: true, minimum_tests: 5}]
    timeout_seconds: 15
YAML
for scenario in faulty:1 fixed:0 unknown:2; do
    variant=${scenario%:*}
    printf '%s\n' "$variant" > "$repo/variant.txt"
    result=0
    "$QG" --root "$repo" check --worktree --profile full --format json \
        --output-dir "$case_dir/$variant" > "$case_dir/$variant.json" || result=$?
    test "$result" -eq "${scenario#*:}"
done
printf '%s\n' "$case_dir"
```
