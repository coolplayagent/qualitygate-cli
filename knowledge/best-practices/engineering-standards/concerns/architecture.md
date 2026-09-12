# Architecture and boundary rules

Architecture rules need declared intent. A dependency graph can prove that a
relationship exists; it cannot prove that the relationship violates an intended
design until the repository supplies an explicit boundary policy.

## Lifecycle input

The secure-design-review contract applies when a change affects a trust
boundary, externally reachable flow, identity, secret, sensitive data, or
material dependency. It requires a design record, threat model, abuse-case
tests, and scoped system evidence.

The Java module-boundary rule is narrower: it evaluates configured Maven
module directions from resolved project facts. It does not claim to evaluate
cloud resources, service topology, or a design document automatically.

## Source synthesis

AWS asks teams to assess risk early, model threats, and attach testable abuse
cases to the backlog.[1] Azure ties security baselines to architecture,
identity, encryption, review, and scans.[2] Alibaba and Meta guide small
interfaces and controlled dependencies for Java and C++ respectively.[3] [4]

## Gate behavior

- Contradictory or missing boundary facts are incomplete.
- A design-review input is not a source-text rule.
- An architecture warning must identify the affected boundary and evidence.
- A blocking decision needs an adopted policy, not only a vendor guideline.

## Sources

1. AWS, [Application risk assessments for secure software design](https://docs.aws.amazon.com/wellarchitected/latest/devops-guidance/qa.st.3-use-application-risk-assessments-for-secure-software-design.html).
2. Microsoft Azure, [Security maturity model](https://learn.microsoft.com/en-us/azure/well-architected/security/maturity-model).
3. Alibaba, [Alibaba Java Coding Guidelines](https://github.com/alibaba/Alibaba-Java-Coding-Guidelines).
4. Meta, [Velox coding style](https://github.com/facebookincubator/velox/blob/main/CODING_STYLE.md).

[1]: https://docs.aws.amazon.com/wellarchitected/latest/devops-guidance/qa.st.3-use-application-risk-assessments-for-secure-software-design.html
[2]: https://learn.microsoft.com/en-us/azure/well-architected/security/maturity-model
[3]: https://github.com/alibaba/Alibaba-Java-Coding-Guidelines
[4]: https://github.com/facebookincubator/velox/blob/main/CODING_STYLE.md
