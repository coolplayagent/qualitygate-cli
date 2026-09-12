# Java practice lane

## Source coverage

Alibaba provides severity-tiered guidance covering code, concurrency, exceptions,
dependencies, project conventions, and security. Its P3C project shows how a
subset can be implemented through PMD and IDE inspections.[1] [2] Google Java
supplies enforceable source-file and naming conventions, while Huawei CodeArts
provides configurable Java rule-set categories.[3] [4]

## Current rule inputs

| Qualitygate rule | Evidence | Decision boundary |
| --- | --- | --- |
| junit-naming | parsed added JUnit test and annotations | violation only when discovery remains valid |
| module-boundary | resolved Maven project facts and configured directions | violation only for declared forbidden direction |
| used-undeclared | compilation inventory, bytecode usage, dependency facts | violation only with complete analyzer inputs |
| generic tests/comments | syntax spans and configured policy | test style can use a warning before escalation |

## Architecture and security

Treat dependency direction, API compatibility, and stable artifact versions as
architecture controls. Treat authorization, input limits, SQL parameterization,
output encoding, CSRF, thread-pool bounds, lock ordering, and thread-local
cleanup as Java-specific security and reliability questions. These need
semantic adapters or design evidence; naming and source text cannot prove them.

## Performance

Performance claims need a representative benchmark, profile, environment, and
threshold. Alibaba's concurrency guidance makes bounded executors and resource
limits relevant review questions, but it does not justify a generic performance
finding for every Java diff.

## Sources

1. Alibaba, [Alibaba Java Coding Guidelines](https://github.com/alibaba/Alibaba-Java-Coding-Guidelines).
2. Alibaba, [P3C](https://github.com/alibaba/p3c).
3. Google, [Google Java Style Guide](https://google.github.io/styleguide/javaguide.html).
4. Huawei Cloud, [CodeArts Check Rule Set](https://support.huaweicloud.com/intl/en-us/usermanual-codecheck/devcloud_hlp_00116.html).

[1]: https://github.com/alibaba/Alibaba-Java-Coding-Guidelines
[2]: https://github.com/alibaba/p3c
[3]: https://google.github.io/styleguide/javaguide.html
[4]: https://support.huaweicloud.com/intl/en-us/usermanual-codecheck/devcloud_hlp_00116.html
