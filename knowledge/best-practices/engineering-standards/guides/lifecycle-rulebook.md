# Lifecycle practice rules

This rulebook converts reviewed external guidance into inputs that can be
audited by qualitygate. It separates a claim that a rule can prove from a
recommendation that still needs a design record, analyzer report, benchmark, or
production signal. The authoritative machine form is
[the lifecycle matrix](../lifecycle-rule-matrix.yaml).

## Result model

An enforced input evaluates only immutable snapshot evidence and reports a
violation or warning. An evidence-contract input becomes incomplete when a
selected task lacks its required evidence. A planned input is visible for
roadmap and policy discussion but cannot change a verdict. This follows the
same promotion discipline described by Cloudflare: guidance is reviewed before
it becomes enforced.[1]

| Lifecycle stage | Decision to make | Qualitygate input class | Required evidence |
| --- | --- | --- | --- |
| Plan | Is the change material enough to require specialist review? | evidence contract | task scope, changed surface |
| Architecture | Are trust, data, dependency, and performance boundaries explicit? | design evidence | design record, threat model, module graph |
| Implementation | Does changed code meet language-aware source rules? | enforced builtin | snapshot bytes and parsed spans |
| Review | Does the change improve code health and remain understandable? | enforced builtin | diff, commit history, review policy |
| Static analysis | Are targeted issue classes analyzed against the selected bytes? | external report | tool identity, version, scope, report, suppressions |
| Verification | Does the system meet test, correctness, and performance claims? | semantic or benchmark evidence | tests, baseline, environment, threshold |
| Release | Are component and deployment decisions traceable? | evidence contract | inventory, release evidence, policy |
| Operations | Do runtime objectives remain true after deployment? | operational evidence | SLO, telemetry, alerts, follow-up |

The matrix makes the plan and release boundaries concrete: a risk-based
security-verification plan records the selected system scope, risk
classification, requirements, and verification plan; a release contract records
the component inventory, selected snapshot, artifact digest, provenance, and
release policy. These are selected evidence contracts, not universal default
checks. [NIST SSDF](https://csrc.nist.gov/pubs/sp/800/218/final) and
[OWASP ASVS](https://owasp.org/www-project-application-security-verification-standard/)
supply the former's vocabulary, while [SLSA](https://slsa.dev/spec/v1.2/) and
[OpenSSF](https://scorecard.dev/) supply the latter's provenance and posture
vocabulary.

Google frames review as a code-health decision that balances forward progress
with technical evidence, rather than a demand for perfection.[2] AWS treats
review as shared change management with explicit completion criteria.[3] Those
principles explain why diff-size stays a warning: it narrows review scope but
does not prove performance, security, or correctness.

## Language lanes

| Lane | Primary source families | Current qualitygate status | Escalation boundary |
| --- | --- | --- | --- |
| Java | Alibaba guidelines and P3C, Google Java, Huawei CodeArts | junit-naming, module-boundary, and used-undeclared are enforced when Maven facts are complete | authorization, SQL, thread pools, and API compatibility need semantic or task evidence |
| Python | Google Python, Huawei CodeArts, Meta static-analysis practice | pytest-naming and generic test/comment checks use parsed source | exception semantics, taint analysis, and dependency closure need a configured report |
| Rust | Rust API Guidelines, Cloudflare language governance | syntax and generic test rules run; public-API review is planned | an adapter needs an adopted compatibility policy and rustdoc/API diff |
| C++ and CUDA | Google C++, NVIDIA CUDA, Meta Velox/Infer, Huawei CodeArts | archival and evidence contracts are available | include correctness, memory, concurrency, numerical behavior, and profiles need language tools |
| TypeScript and Go | Google TypeScript, Huawei CodeArts, Cloudflare | syntax discovery exists; static gate is planned | type/lint report schema and selected-snapshot binding are required |

The [Java](../languages/java.md), [Python](../languages/python.md),
[Rust](../languages/rust.md), [C++ and CUDA](../languages/cpp-cuda.md), and
[TypeScript and Go](../languages/typescript-go.md) notes define scope and
evidence for each lane. A guide for one language is never silently applied to another:
Alibaba's mandatory labels are Java-specific, while CUDA advice requires a
comparable GPU workload and environment.[4] [5]

## Architecture and quality-specialty lanes

| Specialty | Rule question | Evidence that can decide it | Why source text alone is insufficient |
| --- | --- | --- | --- |
| Architecture | Does a new dependency or trust boundary violate an explicit direction? | resolved project facts plus configured boundary policy | cloud guidance cannot infer local module intent |
| Secure design | Have abuse cases and threats become requirements and tests? | design record, threat model, abuse-case tests | source scans cannot prove the design considered omitted flows |
| Static security | Did an analyzer inspect the selected snapshot with a valid configuration? | versioned analyzer report, scope, suppression record | a clean partial report can hide unscanned code |
| Performance | Does a measured workload remain within agreed targets? | comparable baseline, environment, threshold, profile or telemetry | line count and code style are not performance measurements |
| Operations | Are reliability and security objectives observed after release? | SLO, telemetry, alert policy, incident actions | a local Git snapshot has no production truth |
| Supply chain | Can a consumer associate an artifact with reviewed source and declared components? | inventory, artifact digest, build provenance, release policy | an SBOM, signature, or aggregate score alone is not a release verdict |

AWS directs teams to turn risk assessment and threat modeling into backlog
items with tests.[6] Azure calls for a security baseline integrated through the
development lifecycle, code review, and automated scans.[7] NVIDIA's APOD
cycle couples measurement, parallelization, optimization, and deployment, and
requires numerical validation alongside speedups.[5] These sources support
evidence contracts instead of unsupported source-only findings.

## Packaged rule inputs

Each packaged YAML definition carries standard_refs and lifecycle_inputs. The
catalog rejects a rule when either side is absent, a source is unarchived, an
input is not enforced, a rule ID does not match, or the two source sets differ.
The rules list JSON output exposes the resulting metadata with the definition.

| Packaged rule | Lifecycle input | Outcome | Deliberate limit |
| --- | --- | --- | --- |
| line-ending | source-format-hygiene | violation | LF preservation is not a formatter or style proof |
| commit-message | change-subject-traceability | violation | messages do not replace review |
| diff-size | focused-change-review | warning | size is not a performance proxy |
| comment-language | explanatory-comments | warning | language detection is heuristic |
| test-naming, junit-naming, pytest-naming | test convention inputs | violation | framework discovery is authoritative |
| parameterized-tests | test-variation-design | warning | similar syntax does not prove equivalent semantics |
| ai-code-traceability | ai-change-provenance | violation or incomplete evidence | declarations require signed, bound evidence |
| module-boundary, used-undeclared | Java architecture and dependency inputs | violation or incomplete evidence | resolved Maven and bytecode facts are required |

## Adoption sequence

1. Add or revise a source in [the registry](../registry.yaml) with its
   language, lifecycle scope, authority, and caveat.
2. Add a matrix input with an explicit status, evidence list, applicability,
   and critical-adoption note.
3. Promote it to enforced only after a deterministic rule implementation and
   a source-set mapping exist.
4. Use a warning while collecting false-positive and operational cost evidence.
5. Promote a warning to a blocking violation only through an adopted repository
   policy and a reviewable change.

This sequence prevents a vendor document, a model suggestion, or a tool's
default severity from silently changing a repository's merge policy.

## Sources

1. Cloudflare, [How Cloudflare enforces engineering standards using AI](https://blog.cloudflare.com/engineering-standards-enforcement/), 2026.
2. Google, [The Standard of Code Review](https://google.github.io/eng-practices/review/reviewer/standard.html).
3. AWS, [Code review — DevOps Guidance](https://docs.aws.amazon.com/wellarchitected/latest/devops-guidance/code-review.html).
4. Alibaba, [Alibaba Java Coding Guidelines](https://github.com/alibaba/Alibaba-Java-Coding-Guidelines).
5. NVIDIA, [CUDA C++ Best Practices Guide](https://docs.nvidia.com/cuda/cuda-c-best-practices-guide/).
6. AWS, [Application risk assessments for secure software design](https://docs.aws.amazon.com/wellarchitected/latest/devops-guidance/qa.st.3-use-application-risk-assessments-for-secure-software-design.html).
7. Microsoft Azure, [Security maturity model](https://learn.microsoft.com/en-us/azure/well-architected/security/maturity-model).

[1]: https://blog.cloudflare.com/engineering-standards-enforcement/
[2]: https://google.github.io/eng-practices/review/reviewer/standard.html
[3]: https://docs.aws.amazon.com/wellarchitected/latest/devops-guidance/code-review.html
[4]: https://github.com/alibaba/Alibaba-Java-Coding-Guidelines
[5]: https://docs.nvidia.com/cuda/cuda-c-best-practices-guide/
[6]: https://docs.aws.amazon.com/wellarchitected/latest/devops-guidance/qa.st.3-use-application-risk-assessments-for-secure-software-design.html
[7]: https://learn.microsoft.com/en-us/azure/well-architected/security/maturity-model
