# AWS

Sources: [AWS Well-Architected security lifecycle](https://docs.aws.amazon.com/wellarchitected/latest/framework/sec-11.html),
[code review](https://docs.aws.amazon.com/wellarchitected/latest/devops-guidance/code-review.html),
[application risk assessment](https://docs.aws.amazon.com/wellarchitected/latest/devops-guidance/qa.st.3-use-application-risk-assessments-for-secure-software-design.html),
[performance efficiency](https://docs.aws.amazon.com/wellarchitected/latest/framework/a-performance-efficiency.html),
and [software component management](https://docs.aws.amazon.com/wellarchitected/latest/devops-guidance/software-component-management.html).

Authority: AWS Well-Architected and DevOps guidance.

## Rule inputs

- Automate security testing, conduct code reviews, manage dependencies, and
  normalize findings such as SARIF so results remain comparable.
- Review architecture early, threat-model the design, turn risks into
  requirements, and attach tests to abuse or misuse cases.
- Use KPIs, benchmarks, load tests, monitoring, automation, and recurring
  review to maintain performance efficiency.
- Treat peer review, completion criteria, ownership, inventory, and dependency
  relationships as separate change-management and supply-chain inputs.

Qualitygate maps dependency evidence and report normalization to existing
checks. Threat models, performance baselines, and review ownership require a
task or design-record contract; missing required evidence stays incomplete.
