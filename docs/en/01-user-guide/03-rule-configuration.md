# Rule configuration

[简体中文](../../zh/01-user-guide/03-rule-configuration.md) · [Volume index](README.md)

Start with read-only discovery:

```bash
qualitygate --root . rules categories --format json
qualitygate --root . rules list --source all --format json
qualitygate --root . rules describe RULE_ID --format json
qualitygate --root . config --show --format json
```

The catalog includes active rules, unselected built-in packages, and project
rules. `enabled`, `source`, `language`, capability requirements, parameters,
severity, and limitations are separate fields. Listing or assigning categories
does not enable checks or approve policy.

Candidate mutations are explicit:

```bash
qualitygate --root . rules enable RULE_ID
qualitygate --root . rules disable RULE_ID
qualitygate --root . rules configure RULE_ID --set key=value
```

Writes validate the complete candidate and replace configuration atomically.
Invalid parameters, duplicate IDs, unknown capabilities, and unreadable rule
packages are errors. A syntactically valid candidate still needs the team's
normal review and adoption process.

Project rules live under `qualitygate/rules` by default. Author them from the
exported schema, validate a candidate, and only then publish it:

```bash
qualitygate rules schema
qualitygate rules validate candidate.yaml
qualitygate rules generate --input reviewed-source.md
```

Generated rules preserve source hashes and declared capability limits, but
generation is not normative review. Pattern rules are bounded signals: for
example, a credential-like literal or unsafe-looking call is not proof of a
vulnerability. Required facts that are missing, stale, malformed, or outside an
adapter's supported capability produce incomplete validation.

Use file contracts for required or size-bounded files, and report ratchets for
external analyzer debt. Neither is an invented built-in rule ID.
