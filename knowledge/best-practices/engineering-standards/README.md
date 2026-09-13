# External engineering standards archive

This directory records reviewed public guidance that informs qualitygate rule
definitions. The archive keeps short, dated summaries and stable source links;
it does not copy full pages or PDFs.

The machine-readable index is [registry.yaml](registry.yaml); the rule-to-stage
contracts are in [lifecycle-rule-matrix.yaml](lifecycle-rule-matrix.yaml).
Each source note records the company, authority level, topics, and the evidence
that qualitygate can reasonably consume. The source IDs in built-in rule files
and their lifecycle input IDs are checked against both files when the rule
catalog loads.

The archive covers five concerns:

- coding: naming, formatting, comments, exceptions, and API shape;
- architecture: boundaries, dependency direction, threat modeling, and public surface;
- security: authorization, input/output handling, secure design, and static analysis;
- performance: bounded work, measurement, benchmarking, parallelism, memory and load;
- gate: automated review, report normalization, change evidence, and CI feedback.

These references are inputs to policy design. They do not silently impose a
company's complete internal standard on another repository. A rule must state
its applicability, evidence source, severity, and whether a result is a
violation or incomplete evidence.

Source notes:

- [Alibaba](sources/alibaba.md)
- [Google](sources/google.md)
- [Huawei](sources/huawei.md)
- [NVIDIA](sources/nvidia.md)
- [AWS](sources/aws.md)
- [Azure](sources/azure.md)
- [Cloudflare](sources/cloudflare.md)
- [Meta](sources/meta.md)
- [Go Project supplemental language guidance](sources/go.md)
- [Rust Project supplemental language guidance](sources/rust.md)
- [cross-industry security and supply-chain foundations](sources/foundations.md)

The archive is organized for review from three directions:

- [lifecycle rulebook](guides/lifecycle-rulebook.md) and
  [critical adoption](guides/critical-adoption.md), plus the
  [research method](guides/research-method.md);
- [language lanes](languages/java.md) for Java,
  [Python](languages/python.md), [Rust](languages/rust.md),
  [C++ and CUDA](languages/cpp-cuda.md), and
  [TypeScript and Go](languages/typescript-go.md);
- [architecture](concerns/architecture.md),
  [secure coding](concerns/secure-coding.md),
  [performance](concerns/performance.md), and
  [static gates](concerns/static-gates.md), and
  [supply chain and release evidence](concerns/supply-chain.md).

The research snapshot was reviewed on 2026-09-13. Recheck live pages before
changing a rule, especially where a vendor guide is updated continuously.
