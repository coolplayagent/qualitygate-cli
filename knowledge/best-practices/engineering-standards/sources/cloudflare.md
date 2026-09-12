# Cloudflare

Source: [How Cloudflare enforces engineering standards using AI](https://blog.cloudflare.com/engineering-standards-enforcement/)

Authority: Cloudflare engineering publication, describing its Codex and review
workflow.

## Rule inputs

- Store guidance as a governed, retrievable body of standards rather than
  leaving it scattered across conversations and individual memory.
- Reuse the same standards in technical design review, code review, incident
  review, security, and compliance workflows.
- Convert expert guidance into actionable checks while retaining human review
  for judgment and exceptions.
- Promote an approved standard to enforced only after teams have time and
  tooling to absorb it; approved guidance remains nonblocking.

Qualitygate uses this source to justify the standards registry, source IDs,
reviewable diagnostics, and merge-gate mapping. It does not treat AI output as
proof of a violation; a check must retain deterministic evidence or remain
incomplete.
