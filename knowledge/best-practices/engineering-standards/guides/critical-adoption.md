# Critical adoption and conflicts

External standards are useful because they record hard-won experience. They
also conflict in scope, maturity, language, tools, and enforcement style. This
archive adopts a claim only at the strength supported by qualitygate evidence.

## Precedence

1. A repository's explicit security, legal, or task contract governs when it is
   more specific and does not reduce a non-negotiable safety requirement.
2. A language or framework's discovery, compiler, and runtime semantics govern
   implementation details.
3. A reviewed external source informs a compatible control.
4. Personal style is advisory. Existing local style wins only when no stronger
   rule applies and code health does not decrease.

This ordering follows Google's distinction between evidence and preference in
review.[1] It also keeps Alibaba's Java-specific mandatory labels from becoming
universal Rust, Python, or C++ requirements.

## Conflicts and decisions

| Tension | Critical reading | Qualitygate decision |
| --- | --- | --- |
| Perfection versus delivery | Google favors demonstrable code-health improvement without blocking every polish item; Cloudflare distinguishes approved guidance from enforced requirements.[1] [2] | Only deterministic, adopted requirements block. Style and heuristic findings begin as warnings. |
| Vendor style versus repository style | Naming, whitespace, import order, and comments differ across languages and projects. | Use language adapters and project policy. Do not impose an external format rule without a compatible formatter or parser. |
| Static analysis versus complete assurance | Meta's Zoncolan deliberately targets issue classes that static analysis can detect and validates new rules for precision.[3] | Require tool identity, snapshot binding, report completeness, and suppression evidence. A missing or partial report is incomplete, not pass. |
| Security versus speed | Fast paths, caching, concurrency, and relaxed checks can weaken authorization, validation, or auditability. | Security boundaries, authorization, data protection, and input/output controls outrank an unmeasured speed claim. Any exception needs a design record and compensating controls. |
| Performance optimization versus correctness | NVIDIA notes that parallel numerical results may differ from sequential output and calls for measurement and validation.[4] | Require a representative baseline, tolerances or reference comparison, target hardware/environment, and a profile. Never use diff size as a performance gate. |
| Production realism versus customer risk | Azure recommends production-like testing and acknowledges live-test risk and rollback needs.[5] | Production experiments are evidence contracts, not an automatic local test. Scope, exposure, safeguards, and rollback are mandatory inputs. |
| Cloud architecture versus local module rules | AWS and Azure provide workload design criteria; a Rust CLI can only inspect explicit local facts. | module-boundary reports only a configured Maven direction with resolved facts. Cloud recommendations remain design-review prompts. |
| AI review versus deterministic proof | Cloudflare uses governed standards throughout the lifecycle but promotes requirements before blocking.[2] | AI findings can direct review. A qualitygate violation requires deterministic snapshot evidence or verified external evidence. |
| Tool popularity versus portability | Huawei provides language-specific rule sets; Alibaba P3C implements a subset of Java rules; Meta tools have their own language and model requirements.[6] [7] | Preserve rule identity and report provenance. Do not claim a vendor tool's category is a portable parser or severity policy. |

## Promotion criteria

An input may move from planned to evidence-contract when its evidence schema,
task applicability, and failure state are defined. It may move from
evidence-contract to enforced only when all of these are true:

- a bounded implementation can evaluate immutable evidence;
- language, project, and snapshot scope are explicit;
- expected false positives and suppressions have a reviewable policy;
- missing evidence produces incomplete rather than success;
- the repository adopts severity and exception handling explicitly.

Cloudflare's separation of approved and enforced standards is a useful model
for this promotion process.[2] It prevents a new guide from blocking merges
before teams have tooling and context to apply it safely.

## Source status limits

The sources document public practices, not contracts with this repository.
P3C, Infer, and project coding guides demonstrate possible implementations;
they do not establish that their current tools, supported languages, versions,
or severities apply here. The archived Pysa GitHub Action is historical
evidence of CI integration and is not recommended as a current default.

## Sources

1. Google, [The Standard of Code Review](https://google.github.io/eng-practices/review/reviewer/standard.html).
2. Cloudflare, [How Cloudflare enforces engineering standards using AI](https://blog.cloudflare.com/engineering-standards-enforcement/), 2026.
3. Meta, [Zoncolan: Using static analysis to prevent security issues](https://engineering.fb.com/2019/08/15/security/zoncolan/).
4. NVIDIA, [CUDA C++ Best Practices Guide](https://docs.nvidia.com/cuda/cuda-c-best-practices-guide/).
5. Microsoft Azure, [Architecture strategies for performance testing](https://learn.microsoft.com/en-us/azure/well-architected/performance-efficiency/performance-test).
6. Huawei Cloud, [CodeArts Check Rule Set](https://support.huaweicloud.com/intl/en-us/usermanual-codecheck/devcloud_hlp_00116.html).
7. Alibaba, [P3C](https://github.com/alibaba/p3c).

[1]: https://google.github.io/eng-practices/review/reviewer/standard.html
[2]: https://blog.cloudflare.com/engineering-standards-enforcement/
[3]: https://engineering.fb.com/2019/08/15/security/zoncolan/
[4]: https://docs.nvidia.com/cuda/cuda-c-best-practices-guide/
[5]: https://learn.microsoft.com/en-us/azure/well-architected/performance-efficiency/performance-test
[6]: https://support.huaweicloud.com/intl/en-us/usermanual-codecheck/devcloud_hlp_00116.html
[7]: https://github.com/alibaba/p3c
