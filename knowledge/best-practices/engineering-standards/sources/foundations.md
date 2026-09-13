# Cross-industry security and supply-chain foundations

This note complements the company-specific material in this archive. These
sources supply a common vocabulary for controls that span languages and cloud
providers; they do not override a repository's explicit risk policy.

## NIST SSDF

[NIST SP 800-218 SSDF 1.1](https://csrc.nist.gov/pubs/sp/800/218/final) is a
final, outcome-oriented secure-development framework. It explains why secure
practices must be integrated throughout an SDLC and includes requirements,
risk/design decisions, secure build environments, component provenance, and
vulnerability response as related concerns.

Qualitygate uses SSDF as support for a risk-based planning contract and a
release provenance contract. SSDF deliberately does not prescribe one tool,
parser, threshold, or CI implementation, so it cannot by itself justify a
source-only violation.

## OWASP ASVS

[OWASP ASVS 5.0.0](https://owasp.org/www-project-application-security-verification-standard/)
is a versioned list of web-application security requirements and verification
levels. When a task cites an ASVS control, it must retain the versioned
identifier (for example, `v5.0.0-1.2.5`) rather than assuming that an unpinned
"latest" identifier is stable.

ASVS is scoped to web applications and services. It helps select
risk-appropriate requirements and tests, but does not prove that every service,
desktop program, embedded component, or cloud workload has the same threat
model. Qualitygate therefore records security requirements, scope, and a
verification plan as evidence instead of inventing generic pass/fail findings.

## SLSA and OpenSSF

[SLSA v1.2](https://slsa.dev/spec/v1.2/) is the current approved specification
in this archive. It defines separate source and build tracks, progressively
stronger guarantees, and recommended provenance/verification formats. Its
evidence is meaningful only when a consumer declares what provenance it expects
and verifies the artifact against those expectations.

[OpenSSF developer resources](https://best.openssf.org/developers.html) and
[OpenSSF Scorecard](https://scorecard.dev/) provide practical discovery and
automated posture checks for source control, dependencies, builds, signing,
testing, and maintenance. Scorecard's aggregate result is not portable release
severity: a project must retain the tool version, selected snapshot, component
inventory, individual findings, and approved exceptions before using it as gate
evidence.

Qualitygate's release contract consequently requires a component inventory,
artifact digest, provenance, selected snapshot, and adopted release policy. A
signed provenance record says how an artifact was built; it does not establish
absence of vulnerabilities, authorization correctness, or fitness for a
particular deployment.
