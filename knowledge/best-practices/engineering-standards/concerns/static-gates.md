# Static gates and review evidence

Static gates are a lifecycle layer, not a universal proof of quality. A useful
gate preserves rule identity, analyzer identity, version, snapshot scope,
findings, and suppression context, then tells a violation apart from incomplete
execution.

## Rule classes

| Class | Examples | Typical decision |
| --- | --- | --- |
| deterministic source | line endings, configured commit pattern | violation with snapshot bytes |
| parsed structure | test discovery, comment spans, dependency direction | violation or warning with adapter facts |
| external analyzer | SAST, taint, cost, lint, coverage | incomplete when inputs or reports are not comparable |
| design or operations | threat model, SLO, benchmark | evidence contract, not source-only rule |

Huawei separates language-specific critical, general, and comprehensive rule
sets and allows issue-level configuration.[1] Alibaba P3C demonstrates a
subset of guideline rules implemented in PMD and IDE checks.[2] Meta describes
high-confidence static rules that are validated and applied before shipping,
while noting their bounded issue coverage.[3] Cloudflare separates advisory
guidance from enforced standards and uses fast linters for mechanically
verifiable language rules.[4] OpenSSF Scorecard adds repository-level
source/build/dependency posture checks, but its aggregate score remains a
prioritization signal rather than a portable severity mapping.[5]

## Gate behavior

The source archive uses this evidence hierarchy:

1. deterministic source evidence may block only when adopted;
2. parser or project-fact evidence may block only within its supported scope;
3. report-backed results need complete report provenance;
4. missing evidence is incomplete, never a silent success;
5. advisory findings remain visible without changing a pass into a violation.

## Sources

1. Huawei Cloud, [CodeArts Check Rule Set](https://support.huaweicloud.com/intl/en-us/usermanual-codecheck/devcloud_hlp_00116.html).
2. Alibaba, [P3C](https://github.com/alibaba/p3c).
3. Meta, [Zoncolan: Using static analysis to prevent security issues](https://engineering.fb.com/2019/08/15/security/zoncolan/).
4. Cloudflare, [How Cloudflare enforces engineering standards using AI](https://blog.cloudflare.com/engineering-standards-enforcement/).
5. OpenSSF, [Scorecard](https://scorecard.dev/).

[1]: https://support.huaweicloud.com/intl/en-us/usermanual-codecheck/devcloud_hlp_00116.html
[2]: https://github.com/alibaba/p3c
[3]: https://engineering.fb.com/2019/08/15/security/zoncolan/
[4]: https://blog.cloudflare.com/engineering-standards-enforcement/
[5]: https://scorecard.dev/
