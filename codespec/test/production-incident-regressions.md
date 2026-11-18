# 事故防护验证记录

2026-11-18，在 Linux x86_64、Rust 1.97.1 上验证了异常吞没、计数溢出、路径别名、门槛舍入和模块依赖环。案例编号对应[事故与历史缺陷研究记录](../../knowledge/best-practices/engineering-standards/guides/production-incident-patterns.md)。以下是本地受控输入的结果。

## IG-08 · Python 异常被吞没

取同一个 `parse(raw)` 函数和约定「非法整数必须向调用者传播 `ValueError`」。异常处理位于新增文件 `handler.py` 第 4 行。对同一 Git 基线只替换第 4、5 行，显式选择两个版本 1 的内置规则。`no-bare-except` 扫描新增 Python 文件，默认 error；`py-bare-except` 扫描变化行，默认 warning。[文件规则](../../skills/qualitygate-cli/references/rules/shared/no-bare-except.yaml) · [变化行规则](../../skills/qualitygate-cli/references/rules/lang-python/py-bare-except.yaml)

| 处理子句及其内容 | 文件规则 | 变化行规则 | 完整交付门禁 |
| --- | --- | --- | --- |
| `except:` / `pass` | fail，第 4 行 | fail，第 4 行 | fail，退出 1，阻断项为 `no-bare-except` |
| `except ValueError:` / `raise` | pass | pass | pass，退出 0 |
| `except ValueError:` / `pass` | pass | pass | pass，退出 0，漏掉了约定要求传播的异常 |
| 只启用变化行规则，`except:` / `pass` | 未启用 | fail，第 4 行 | pass，退出 0，保留警告 |

各次报告均为 `profile: full`、`scope: delivery`、`gate.complete: true`，待执行交付项为空。双规则分别观测 1 个文件和 5 行。两种显式捕获的报告结论均为「在已验证形态下未发现问题」。工具读取语句形态，未执行 Python 的异常分支。若要把这个行为约定设为门禁，项目测试应向函数传入确定的非法整数，并断言调用者收到指定异常。第三组是应能使该测试失败的反例。

**运行身份。** 源码基线为 `385ce2702db4f8c41d0c644a76403770838df7a3`；开发 CLI 1.3.0 的 SHA-256 为 `e4a3097455b33f0d9e0135fae519d2da7d079b2d6bc2d6eac10ac7613e4c6696`。规则从同一 checkout 的 `skills/qualitygate-cli/references/rules` 加载，Git tree 为 `8e5b1be7d47a61abb2c59bb7091599b4e185c913`。双规则策略报告 `trust: local_candidate`，`config_digest` 为 `sha256:3695436a559ab70ba99fdf40020755db3432d2340184fc4f40e31590a0ec6fa0`。

从仓库根目录运行以下 Bash 示例。每次使用新临时目录；Git 作者设置仅用于此次提交。

```bash
qg="$PWD/target/debug/qualitygate"
export QUALITYGATE_BUILTIN_RULES_DIR="$PWD/skills/qualitygate-cli/references/rules"
experiment=$(mktemp -d)
repo="$experiment/repo"
git init -q "$repo"
cat >"$repo/qualitygate.yaml" <<'YAML'
schema_version: 1
rulesets: [shared, lang-python]
rules:
  no-bare-except: {}
  py-bare-except: {}
YAML
git -C "$repo" add qualitygate.yaml
GIT_AUTHOR_DATE=2026-01-01T00:00:00Z GIT_COMMITTER_DATE=2026-01-01T00:00:00Z \
  git -C "$repo" -c user.name=Fixture -c user.email=fixture@example.invalid \
  -c commit.gpgSign=false commit -qm 'fixture policy'
"$qg" --root "$repo" config --show --format json >"$experiment/config.json"

check_handler() {
  printf 'def parse(raw):\n    try:\n        return int(raw)\n    %s\n        %s\n' \
    "$2" "$3" >"$repo/handler.py"
  code=0
  "$qg" --root "$repo" check --worktree --profile full --format json \
    >"$experiment/$1.json" || code=$?
  printf '%s exit %s\n' "$1" "$code"
  jq '{gate, verification, checks: [.checks[] | {id, verdict, severity, diagnostics}]}' \
    "$experiment/$1.json"
  test "$code" -eq "$4"
}
check_handler bare 'except:' pass 1
check_handler explicit 'except ValueError:' raise 0
check_handler swallowed 'except ValueError:' pass 0
```

第四组复用新建临时仓库的步骤，策略的 `rulesets` 只选 `[lang-python]`，`rules` 只保留 `py-bare-except: {}`，再运行 `check_handler bare 'except:' pass 0`。报告中的 `verification` 保留结论、已验证形态、限制与未验证假设。交付范围、已选策略和此次运行的输入限定了结论；其他框架、运行时配置、生产环境与外部工具须另外验证。

## IG-10 至 IG-12 · 三个历史修复

在上述源码基线复跑三条集成回归。每条 Cargo 命令实际执行 1 项，均通过，退出 0。表中另列被测 CLI 对不同输入的退出码。

| 案例 | 验证条件及实测结果 |
| --- | --- |
| IG-10，`e6ff615` 报告计数溢出 | 前一份测试报告达到 `usize::MAX`，再加入非零测试数或覆盖行数。CLI 退出 2，保留已有计数、诊断和两份原始产物，并阻止依赖项启动；恢复正常计数后退出 0。[计数测试](../../tests/execution/report_counts.rs) |
| IG-11，`f330e9c` 输出路径别名 | 同一次执行中的报告、基线或项目输出归一化后重名时，`check` 和 `doctor --probe-tools` 均退出 2，探测与生产命令均未启动。两个不同路径保存完全相同的内容时，保留两项证据，计数为 2。[输出路径测试](../../tests/execution/output_aliases.rs) |
| IG-12，`0eb693d` 覆盖率舍入 | 分别验证 100%、90%、99.9% 的精确边界。启用分支门槛时，恰好达标退出 0，少命中一次退出 1，原始分子、分母仍在报告中；关闭分支要求时，同一输入按行覆盖率通过。[门槛测试](../../tests/coverage.rs) |

```bash
cargo test --all-features --test execution report_counter_overflow_is_incomplete_with_preserved_evidence_and_blocked_dependents
cargo test --all-features --test execution aliased_outputs_are_rejected_before_probes_and_distinct_reports_still_count_independently
cargo test --all-features --test coverage coverage_thresholds_use_exact_counts_before_rounding_percentages
```

这三组证据来自 Qualitygate 自身的配置、报告和门槛实现。采用方的计数器、输出命名和业务阈值各有独立契约，仍需项目测试覆盖。

## IG-17 · Rust 模块依赖环

本仓 `tests/quality` 中的架构检查从生产 Rust 入口解析源码引用，按[所有权映射](../../tests/quality/architecture_owners.rs)把模块归到能力 owner。对跨 owner 引用，它检查方向并查找有向环。下表记录[架构测试](../../tests/quality/architecture.rs)实际送入同一个 `Graph::source` 的合成源码。

| 输入 | 检测结果 |
| --- | --- |
| `domain → interfaces → domain` | 报直接环，并指出禁止的 `domain → interfaces` 引用。 |
| `application → config → domain → application` | 报含起止点的三 owner 闭环，并将禁止依赖定位到夹具 `domain.rs` 第 1 行。 |
| `interfaces` 同时引用 `application` 和 `config`，`application → config → domain` | 4 条跨 owner 边，通过。共享依赖可形成多条路径。 |
| `python_analysis::package_metadata` 与 `python_analysis::project_model` 互相引用 | 两份夹具源码均解析到引用。两个模块都归属 `adapters`，owner 图产生 0 条边、0 项违规。这是需要模块级图的反例。 |

```bash
cargo test --all-features --test quality architecture
```

本轮该过滤条件执行 15 项，全部通过，含现有直接环反例及新增的间接环、共享依赖、同 owner 反例。生产入口检查生成的 `target/architecture/report.json` 记录 297 个源码文件、2,464 条带文件与行号的跨 owner 边记录、0 项违规，SHA-256 为 `0adc77787487026ee604cacdb40cfb6dee56d506af5b009d999a24938c89612e`。重复引用位置可以属于同一对 owner。报告保留逐文件源码摘要。上述结论限于仓库文档列出的 Rust 源码形式和所有权粒度；接入其他项目需声明各自的入口、依赖解析方式和检查粒度。[架构契约](../../docs/zh/03-architecture/03-development-view.md)

### 在 Rust 模块间阻断已知反向导入

对同属 `layers` 的三个模块，在临时策略中仅检查 `beta.rs` 的变更导入，禁止其通过示例中的 `use` 形式依赖 `alpha`。基线已有 `alpha → beta`，每次仅改动 `beta.rs`。先用 Rust 1.97.1 编译三种输入，三种均编译成功。随后用内建 `import-boundary` 执行完整交付检查。[规则声明](../../skills/qualitygate-cli/references/rules/shared/import-boundary.yaml)

| 变更 | 模块关系 | 门禁实测 |
| --- | --- | --- |
| `use crate::layers::gamma::Gamma` | `alpha → beta → gamma` | 完整 pass，退出 0，检查了一个变更导入。 |
| `use crate::layers::alpha::Alpha` | `alpha → beta → alpha` | 完整 fail，退出 1，诊断位于 `src/layers/beta.rs:1`，保留语言、命中模式和复查命令。 |
| 类型别名直接写 `crate::layers::alpha::Alpha` | 同一个 `alpha → beta → alpha` 闭环 | 完整 pass，退出 0，变更中没有 `use`，检查了零个导入。 |

先声明禁止的边，规则就能在新导入形成闭环前阻断该形态。第三组反例明确了检查范围。任意模块环需要覆盖声明、限定引用和经解析的依赖路径的模块图。生产代码中已有同属 `adapters` 的双向引用，例如 `normalization` 调用 `crapper::parse`，`crapper` 使用 `normalization` 的上下文和计数预算。启用更细粒度的全仓约束前需先审查现存依赖方向。[分派代码](../../src/qualitygate/tool_evidence/normalization.rs) · [解析代码](../../src/qualitygate/tool_evidence/crapper.rs)

CLI 与本页 IG-08 的开发二进制相同。实验 Git 基线为 `ba66bd46dd7856863888854f2726da436c3e4896`，规则资产 SHA-256 为 `e3e567d36bad95ab5649d28b463cc528f41722387b50f0db490786a0db3e1bc0`，策略的 `config_digest` 为 `sha256:bd1d629d658473109e6b2c281043acb2addcad1b52306779fbec076ee3661243`。失败用例诊断指纹为 `sha256:dd50ad96a07bf51825d626dce478df32c69808bac6be2405a365b2f898c5a91d`。

从本仓根目录运行，Git、Rust 1.97.1、jq 和已经构建的开发 CLI 须可用。

```bash
set -eu
qg="$PWD/target/debug/qualitygate"
export QUALITYGATE_BUILTIN_RULES_DIR="$PWD/skills/qualitygate-cli/references/rules"
experiment=$(mktemp -d)
repo="$experiment/repo"
git init -q "$repo"
mkdir -p "$repo/src/layers"
cat >"$repo/qualitygate.yaml" <<'YAML'
schema_version: 1
rulesets: [shared]
rules:
  import-boundary:
    enabled: true
    required: true
    severity: error
    parameters:
      languages: [rust]
      paths: [src/layers/beta.rs]
      forbidden_imports:
        rust: ['crate::layers::alpha(::|;)']
YAML
printf 'pub mod layers;\n' >"$repo/src/lib.rs"
printf 'pub mod alpha;\npub mod beta;\npub mod gamma;\n' >"$repo/src/layers/mod.rs"
printf 'use crate::layers::beta::Beta;\npub struct Alpha(pub Beta);\n' >"$repo/src/layers/alpha.rs"
printf 'pub struct Beta;\n' >"$repo/src/layers/beta.rs"
printf 'pub struct Gamma;\n' >"$repo/src/layers/gamma.rs"
git -C "$repo" add qualitygate.yaml src
GIT_AUTHOR_DATE=2026-01-01T00:00:00Z GIT_COMMITTER_DATE=2026-01-01T00:00:00Z \
  git -C "$repo" -c user.name=Fixture -c user.email=fixture@example.invalid \
  -c commit.gpgSign=false commit -qm 'reviewed import direction'
"$qg" --root "$repo" rules describe import-boundary --format json >"$experiment/rule.json"

run_direction() {
  rustc --edition=2024 --crate-type lib "$repo/src/lib.rs" -o "$experiment/libfixture.rlib"
  code=0
  "$qg" --root "$repo" check --worktree --profile full --format json \
    >"$experiment/$1.json" || code=$?
  jq '{profile, scope, gate, checks: [.checks[] | {id, verdict, diagnostics, matched_entities}]}' \
    "$experiment/$1.json"
  test "$code" -eq "$2"
}
printf 'use crate::layers::gamma::Gamma;\npub struct Beta(pub Gamma);\n' >"$repo/src/layers/beta.rs"
run_direction allowed 0
printf 'use crate::layers::alpha::Alpha;\npub struct Beta;\npub type Upstream = Alpha;\n' \
  >"$repo/src/layers/beta.rs"
run_direction import-cycle 1
printf 'pub struct Beta;\npub type Upstream = crate::layers::alpha::Alpha;\n' \
  >"$repo/src/layers/beta.rs"
run_direction qualified-cycle 0
```

## 工具证据的独立性与有效期

对基于覆盖率计算的 Rust 函数指标，采集前须核对覆盖率生产者、文件清单和内容摘要。两个单元测试验证了正常的嵌套 LCOV 清单，以及同尺寸改写、额外 LCOV、优先级更高的 HTML、缺失或错误来源、只有标准输出和扫描根为普通文件时的拒绝行为。还原同一份清单后再次通过。测试覆盖采集前的输入复核，与集成测试的门禁裁决形成互补。[输入复核单元测试](../../src/qualitygate/gate_execution/fact_revalidation.rs)
