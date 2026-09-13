# Secure coding and threat evidence

Secure coding begins before source exists. The matrix treats threat modeling,
authorization, sensitive-data handling, validation, output encoding, and
abuse-case tests as a design-evidence contract, then uses static analysis for
issue classes that a configured analyzer can actually inspect.

## Source synthesis

Alibaba identifies authorization, input validation, SQL parameterization,
output filtering, CSRF, and replay controls as Java security concerns.[1] NIST
SSDF supplies cross-SDLC secure-development outcomes, while OWASP ASVS supplies
versioned web-application requirements and verification levels.[2] [3] AWS
requires risk assessment and threat-model results to become requirements and
tests.[4] Azure uses a progressively stronger security baseline with review and
automated scans.[5] Meta explains that source-to-sink rules are validated for
the issue classes they are meant to detect.[6]

## Evidence requirements

| Evidence | Why it is required |
| --- | --- |
| design record and threat model | show which flows and boundaries were considered |
| abuse-case tests | show an identified risk has an executable response |
| analyzer identity, version, and configuration | make behavior reproducible |
| selected snapshot and report scope | prevent a result from covering different bytes |
| suppression record | distinguish accepted risk from silently hidden findings |

## Gate behavior

A secure design contract selected by a task becomes incomplete when evidence is
missing. A static-analysis contract becomes incomplete when its report is
partial, stale, unbound, or ambiguous. Neither outcome becomes a pass.

## Sources

1. Alibaba, [Alibaba Java Coding Guidelines](https://github.com/alibaba/Alibaba-Java-Coding-Guidelines).
2. NIST, [SP 800-218 SSDF 1.1](https://csrc.nist.gov/pubs/sp/800/218/final).
3. OWASP, [ASVS 5.0.0](https://owasp.org/www-project-application-security-verification-standard/).
4. AWS, [Application risk assessments for secure software design](https://docs.aws.amazon.com/wellarchitected/latest/devops-guidance/qa.st.3-use-application-risk-assessments-for-secure-software-design.html).
5. Microsoft Azure, [Security maturity model](https://learn.microsoft.com/en-us/azure/wellarchitected/security/maturity-model).
6. Meta, [Zoncolan: Using static analysis to prevent security issues](https://engineering.fb.com/2019/08/15/security/zoncolan/).

[1]: https://github.com/alibaba/Alibaba-Java-Coding-Guidelines
[2]: https://csrc.nist.gov/pubs/sp/800/218/final
[3]: https://owasp.org/www-project-application-security-verification-standard/
[4]: https://docs.aws.amazon.com/wellarchitected/latest/devops-guidance/qa.st.3-use-application-risk-assessments-for-secure-software-design.html
[5]: https://learn.microsoft.com/en-us/azure/well-architected/security/maturity-model
[6]: https://engineering.fb.com/2019/08/15/security/zoncolan/
