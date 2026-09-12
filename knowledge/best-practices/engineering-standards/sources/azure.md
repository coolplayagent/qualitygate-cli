# Azure

Sources: [Azure Well-Architected Framework](https://learn.microsoft.com/en-us/azure/well-architected/)
and [security maturity model](https://learn.microsoft.com/en-us/azure/well-architected/security/maturity-model),
plus [performance testing strategies](https://learn.microsoft.com/en-us/azure/well-architected/performance-efficiency/performance-test).

Authority: Microsoft Azure architecture guidance.

## Rule inputs

- Establish a security baseline during development with code reviews,
  automated scans, input validation, and output encoding.
- Use an identity provider, strict access control, strong encryption, TLS, and
  a patch/catalog process that stays auditable.
- Treat architecture as quality decisions across reliability, security,
  performance efficiency, and operational excellence; review trade-offs.
- Define measurable performance targets, budgets, baselines, comparable
  environments, and controlled progressive testing before a result can be a
  performance gate.

Qualitygate uses this source for `module-boundary`, secure-design review, and
static-gate inputs. It does not infer cloud resource configuration from source
files without an infrastructure adapter and an explicit project scope.
