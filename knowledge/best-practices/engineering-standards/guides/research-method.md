# Research method and source boundaries

## Scope and evidence standard

This archive was rechecked on 2026-09-13 against primary public material. It
covers coding, architecture, secure coding, performance, static gates, and
release/supply-chain practice across the eight requested company families:
Alibaba, Google, Huawei, NVIDIA, AWS, Azure, Cloudflare, and Meta. It also
uses NIST, OWASP, SLSA, OpenSSF, Rust, and Go materials where a cross-company
or language-native control is more authoritative than a company example.

Every archived claim is classified before it reaches a rule input:

| Source form | What it can establish | What Qualitygate still needs |
| --- | --- | --- |
| official guide, product documentation, or online book | rationale, scope, and candidate controls | repository policy plus deterministic implementation or explicit evidence |
| standard or specification | versioned requirement vocabulary and assurance level | selected risk level, applicable scope, and a verification plan |
| project coding guide or analyzer documentation | concrete implementation example and tool limits | language/tool version, complete input scope, and suppression policy |
| benchmark, report, or attestation | evidence about a selected run or artifact | snapshot/artifact binding, environment, threshold, and trust decision |

The source registry intentionally stores short paraphrases and direct links,
not copied vendor manuals. This keeps licenses and changing vendor text out of
the repository while retaining an auditable path to each primary source.

## Review decisions

| Area | Primary families reviewed | Adopted Qualitygate boundary |
| --- | --- | --- |
| language code | Alibaba, Google, Huawei, Meta, Go, Rust | use formatter/parser/compiler facts only in a supported language lane |
| architecture and security | AWS, Azure, Alibaba, Meta, NIST, OWASP | convert risk and threat decisions into scoped requirements and tests; source text alone is not a violation |
| performance | NVIDIA, AWS, Azure, Google SRE | require representative workload, baseline, environment, threshold, and correctness criterion |
| static gates | Huawei, Alibaba P3C, Meta, Cloudflare, OpenSSF | preserve analyzer identity, selected snapshot, scope, findings, and suppression/exception evidence |
| release and operations | AWS, Azure, Google SRE, NIST, SLSA, OpenSSF | distinguish component/provenance evidence from production SLO or security truth |

## Version and freshness controls

- Pin ASVS references to a released version and requirement identifier; the
  project's current page lists ASVS 5.0.0 and warns against unversioned IDs.
- Use SLSA v1.2, not the retired v1.0 levels overview. Its source/build tracks
  are intentionally separate so progress in one does not claim guarantees in
  the other.
- Treat evolving product pages (Cloudflare, AWS, Azure, Huawei, NVIDIA, and
  OpenSSF) as live references. Recheck their wording before increasing a rule's
  severity or scope.
- Treat P3C, Infer, Velox, and language guides as evidence of possible
  implementation patterns, not a portable tool/version recommendation.

These decisions feed [the lifecycle matrix](../lifecycle-rule-matrix.yaml).
The matrix permits a packaged builtin only when it has bounded immutable
evidence. All other source-derived ideas remain planned or evidence-contract
inputs until a repository adopts the required scope and evidence.
