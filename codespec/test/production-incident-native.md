# Linux 崩溃、堆泄漏、句柄泄漏与边界实验

2026-11-18 在 Linux x86_64 的独立 Git 仓库运行。受测程序是可切换正常和缺陷路径的安全 Rust 样例，检查器在父进程观察真实子进程。子进程退出后，父进程才将观测转成 diagnostics 测试计数；runner 同时保留 stderr 原始观测和 stdout 报告。每个场景有独立检查 ID，必需报告至少执行一个测试。[规格 IG-13 至 IG-16](../requirements/production-incident-guards.md) · [完整 Rust 源码](../../fixtures/production-incident-guards/native_resources.rs)

## 同一断言的对照结果

| 检查 ID | 判据 | 缺陷路径 | 修复路径 |
| --- | --- | --- | --- |
| crash | 缺字段必须返回拒绝结果并正常退出 | panic=abort；父进程观察到 SIGABRT（6）和预期 panic 消息 | 退出 0，输出 rejected |
| heap | 在受支持的平台上运行 LeakSanitizer；先用已知泄漏确认插桩实际工作 | 单次遗失 4096 字节，LSan 报 1 次分配，退出 23 | 阳性对照成立，受测进程退出 0 |
| fd | 同一进程重复 16 次打开文件后提前返回；每次操作结束应释放 FD | /proc/self/fd 从 4 增至 20 | 4 → 4 |
| bounds | 对长度 0、7、8、9、usize::MAX 执行公开约定“非空且最多 8 项” | 长度 8 被错误拒绝 | 五个输入均符合约定 |

四项同跑的缺陷快照得到完整 fail（退出 1），修复快照得到完整 pass（退出 0）。故意拿掉 LSan 插桩后，修复快照的 heap 检查因阳性对照缺失而不交报告；stderr 记录原因，Gate 记录 tool_error、Report is empty，整体 incomplete（退出 2）。其余三项仍通过。报告的测试计数是各工作负载的综合断言，样本规模见上表。

| 本次运行 | 快照 content_digest（SHA-256） | CLI stdout JSON 的 SHA-256 |
| --- | --- | --- |
| faulty | 6cb8428d6c9bbca130d5f992b3c48e227c78c20121d9d646f346adeb28ca806c | 7bf21f61eda74eca78ee3015a86ae101d6200df6f76732f997c1d83cc419fe89 |
| fixed | 2c7c9d7185a5f4d5341586e804c8fe7c044a8137b1bef79cd1118bf3a2ec3e53 | 9b8329dd165059679225abaf114e0b5e81694174f53caae6ae633d138583718f |
| 未插桩 | d3abfc615c97a045e8921b8daeb4fa3bf741614c2bb96a80c3c803dd092eedd9 | 97720d87f19548ff927f0101a9e8942567c70fb77c390d4d1c537ebd7393138b |

本次使用已安装的 Qualitygate 1.3.0，二进制 SHA-256 为 c8e402349a10acf4eb0610912c921d51890e9dcbb6ecda88f161188888913da0。rustc 1.101.0-nightly（4ebd25825）的插桩探针 SHA-256 为 448ef458e7f7730d4e036e941e2b50a6e8e7fd299f6ad17d8f12c07a589dbb08。编译路径、Git 身份、运行 ID 和绝对工具路径参与本地记录，复查时比较判据和观测值。

## 复查

从项目根目录启动，令 QG 指向所选 CLI 的绝对路径。在支持 LeakSanitizer 的 Linux x86_64 上使用 Bash、Git 和 nightly rustc。创建临时仓库并在仓库外编译探针，四条检查读取同一工作树快照中的 variant.txt。以下片段生成与实测等价的 YAML 策略。

```bash
set -eu
: "${QG:?请设置 Qualitygate 可执行文件的绝对路径}"
work=$(mktemp -d)
mkdir "$work/repo"
cp fixtures/production-incident-guards/native_resources.rs "$work/repo/"
cd "$work/repo"
git init -q
git -c user.name=fixture -c user.email=fixture@invalid commit --allow-empty -qm baseline
rustc +nightly --edition 2024 -g -C panic=abort -Zsanitizer=leak native_resources.rs -o "$work/native-probe"
rustc +nightly --edition 2024 -g -C panic=abort native_resources.rs -o "$work/native-debug"

policy() {
  echo 'schema_version: 1'
  echo 'checks:'
  for check in crash heap fd bounds; do
    cat <<YAML
  - id: $check
    argv: ["$1", $check]
    timeout_seconds: 15
    tools: [{id: probe, argv: ["$1", "--version"], inputs: [native_resources.rs]}]
    reports: [{path: $check.json, format: diagnostics, from_stdout: true, minimum_tests: 1}]
YAML
  done
  echo 'profiles: {full: {include: [crash, heap, fd, bounds]}}'
}
run_case() {
  printf '%s\n' "$1" > variant.txt
  code=0
  "$QG" --root . check --worktree --profile full --output-dir "$work/$2-report" \
    --format json > "$work/$2.json" || code=$?
  test "$code" -eq "$3"
}
policy "$work/native-probe" > qualitygate.yaml
run_case faulty faulty 1
run_case fixed fixed 0
policy "$work/native-debug" > qualitygate.yaml
run_case fixed uninstrumented 2
```

## core dump 的取证边界

在同一个合成输入上，用 GDB 17.1 启动带调试信息的无插桩程序，停在 SIGABRT 后显式生成 core，重新加载该文件验证终止信号和栈。此次 ELF Build ID 为 d48b67ebcf47a99d3b1b2e4040747465eeffd14d，栈定位到 native_resources::child 的缺字段分支。GDB 对一个 4096 字节的系统映射给出读取警告，用户代码帧可读。本次验证覆盖可调试环境中的转储和符号化；系统自动收集、权限、生产符号库和保存周期需在实际部署验证。[Linux core(5)](https://man7.org/linux/man-pages/man5/core.5.html)

```bash
cd "$work"
(ulimit -f 65536
 env -i PATH=/usr/bin:/bin LC_ALL=C DEBUGINFOD_URLS= gdb -nx -q --batch \
   -ex 'set auto-load off' -ex run -ex 'generate-core-file synthetic.core' \
   --args "$work/native-debug" worker crash faulty)
env -i PATH=/usr/bin:/bin LC_ALL=C DEBUGINFOD_URLS= gdb -nx -q --batch \
  -ex bt "$work/native-debug" "$work/synthetic.core"
readelf -n "$work/native-debug"
```

这组实验只提交源码和摘要。core 包含进程内存，转储实验在私有临时目录中使用合成数据。实际部署应按自身访问和留存策略关联转储与构建身份。堆实验观测不可达分配，FD 实验观测单线程文件错误路径，长度实验观测业务上限。长寿命可达对象、取消时的异步资源回收和原生越界访问仍要分别提供负载、生命周期和插桩证据。[Rust sanitizer 文档](https://doc.rust-lang.org/unstable-book/compiler-flags/sanitizer.html) · [Linux FD 清单](https://man7.org/linux/man-pages/man5/proc_pid_fd.5.html)
