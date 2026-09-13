# Supply-chain and release evidence

Dependency declarations, a lockfile, and a successful build are useful facts,
but none independently proves that a released artifact came from the reviewed
source or that its components are acceptable for a given risk profile.

## Lifecycle inputs

`supply-chain-posture` asks for a versioned analyzer/posture report, component
inventory, selected snapshot, and exception record. The
`release-supply-chain-provenance` input additionally requires the artifact
digest, build provenance, and an adopted release policy. Both are evidence
contracts: a selected required contract with missing or stale evidence is
incomplete, never passed.

## Source synthesis

NIST SSDF includes component provenance and secure development environments in
its SDLC outcomes.[1] SLSA v1.2 separates source and build tracks and requires
consumers to compare provenance with declared expectations.[2] AWS asks teams
to maintain component inventories and governed dependencies,[3] while OpenSSF
offers project-level posture checks that span source, build, dependencies,
testing, and maintenance.[4]

## Critical limits

- An SBOM or inventory describes components; it is not a vulnerability verdict.
- A signed provenance record establishes origin/process evidence, not behavior,
  authorization, or absence of malicious source.
- A posture score aggregates project-specific checks and must not silently map
  to a universal error severity.
- A local working tree cannot substitute for the release snapshot, artifact
  digest, or the policy that selected the evidence.

## Sources

1. NIST, [SP 800-218 SSDF 1.1](https://csrc.nist.gov/pubs/sp/800/218/final).
2. SLSA, [Specification v1.2](https://slsa.dev/spec/v1.2/).
3. AWS, [Software component management](https://docs.aws.amazon.com/wellarchitected/latest/devops-guidance/software-component-management.html).
4. OpenSSF, [Scorecard](https://scorecard.dev/).

[1]: https://csrc.nist.gov/pubs/sp/800/218/final
[2]: https://slsa.dev/spec/v1.2/
[3]: https://docs.aws.amazon.com/wellarchitected/latest/devops-guidance/software-component-management.html
[4]: https://scorecard.dev/
