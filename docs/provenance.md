# Signed agent-run provenance

AI-only scope uses a signed record from a trusted external harness. The CLI authenticates the record, replays its complete sequence of file transformations from the comparison's base, and maps agent participation to final test entities. A missing marker cannot exclude an agent-participating test from checking.

The signer is responsible for observing the whole comparison, identifying actors honestly and protecting its signing key. Replaying signed bytes proves a consistent transformation chain; it cannot independently prove that every real-world edit was observed or that an actor label is truthful. Participation survives later human repairs and does not establish exclusive AI authorship. Declaration text alone supplies no provenance.

## Configure the rule and caller

```yaml
schema_version: 1
rules:
  ai-code-traceability:
    provenance: {evidence_file: runs/trace.json}
    parameters:
      languages: [python, java]
      provenance_scope: ai_only
      marker: {type: comment, name: AI, fields: [author]}
```

For a custom test-method rule, set `applies_to.provenance_scope: ai_only` in its definition and `provenance.evidence_file` in its policy setting. Definitions may require `external_provenance` explicitly. Either requirement needs a verified record, even when no tests match. The custom rule's `when.change` still selects the change kinds to check.

```bash
qualitygate check --worktree --policy-ref "$TRUSTED_COMMIT" \
  --trust-store /trusted/qualitygate/trust.json \
  --evidence-dir /trusted/qualitygate/records --format json
```

Both external locations must be outside the repository. The [shared trust store](manual-acceptance.md#trust-store) authorizes provenance through a separate exact rule-ID list:

```json
{
  "schema_version": 1,
  "repository": "https://github.com/example/orders",
  "max_age_seconds": 86400,
  "keys": [{
    "id": "run-service",
    "public_key": "BASE64_PUBLIC_KEY",
    "provenance_rules": ["ai-code-traceability"]
  }],
  "revoked_records": []
}
```

Replace the illustrative key placeholder with the service's public key. Manual `checks`/`tasks` authorization does not authorize provenance; `provenance_rules` does not authorize manual acceptance. The caller protects the trust store, invocation and policy reference. Configured commands retain the caller's OS permissions.

## Produce the signed comparison

A blocked check reports `metadata.external_provenance.expected_subject`. The harness independently validates that subject against its observed repository, base/output snapshots and trusted policy before signing. Each rule needs a record bound to its own ID. Code, policy, task-contract digest, snapshot mode, comparison or path-filter changes require a matching new record.

The strict JSON payload uses the typed contracts in [domain/provenance.rs](../src/qualitygate/domain/provenance.rs):

| Field | Contract |
|---|---|
| `schema_version` | `1` |
| `record_id` | Unique nonempty record ID, at most 256 bytes; supports revocation |
| `coverage` | `complete_comparison` |
| `subject` | Exact expected repository, snapshot, policy, rule and base `input_content_digest` |
| `issued_at`, `expires_at` | UTC Unix seconds; `issued_at <= now < expires_at`, validity bounded by the trust store |
| `runs` | Unique run IDs, actor identity/kind, optional version and start/end timestamps; agent versions are required |
| `steps` | Ordered run references, input/output content digests and exact file changes |

Each step starts with the previous output digest. Its `changes` contain normalized repository-relative `path`, `before` and `after`. An existing `before` supplies SHA-256 `digest`, byte count `bytes` and `executable`; null means absent. An existing `after` supplies `content_base64` and `executable`; null means deletion. Bytes and executable modes must reproduce every intermediate digest and the final checked snapshot. Each named run must appear in a step; a no-write run can have an empty changes list. File-change entries that have no effect are rejected.

The ledger covers all files in the captured comparison, including human edits, even when a rule selects only particular paths or languages. The CLI applies changes in memory and never executes record contents. Sign exact payload bytes using Ed25519 and DSSE pre-authentication encoding with payload type `application/vnd.qualitygate.agent-provenance.v1+json`. The envelope fields, encoding and one-authorized-signature requirement follow the [manual record protocol](manual-acceptance.md#producing-a-record), using this distinct payload type and signer scope.

Producer helpers are `qualitygate::adapters::provenance::{PAYLOAD_TYPE, subject, file_identity}`, `qualitygate::snapshot::content_digest` and `qualitygate::adapters::attestation::pae`. [Rust CLI fixtures](../tests/provenance.rs) construct and sign real Git snapshot histories, including Python/Java changes, staged selection and repairs. Their fixed private keys are test data only.

## Entity scope and limits

Changed test entities inherit participation from matched predecessors. Same-path symbols are reserved before named cross-file moves and body matches; remaining copies are new entities. Human repairs and unambiguous moves preserve prior agent participation. Multiple possible predecessors make verification incomplete. Existing declarations retain their obligations, and present declarations still require valid fields, even outside the agent-triggered subset.

Participation is relative to the comparison base. The CLI does not infer historical agent activity before that base. Supported test syntax follows the [language adapters](rules.md); changed supported-language files must parse at every recorded checkpoint. Malformed intermediate syntax prevents verification. A final passing snapshot cannot substitute for an unverifiable history. [Git trailer association](git-trailers.md) is a separate declaration binding that can be combined with authenticated AI-only scope.

| Resource | Limit |
|---|---|
| Provenance envelope | 8 MiB per record |
| Shared external inputs | 8 MiB total including trust store and manual/provenance records; at most 128 selected record files |
| Runs and steps | 256 each |
| File changes | 8,192 across the ledger |
| Replayed snapshot | 20,000 files, 2 MiB per file, 12 MiB total |
| Tracked test entities | 50,000 |
| Time | Cooperative 30-second load, signature-verification and replay budgets, plus bounded syntax parsing |

The trust store remains limited to 256 KiB and 64 keys. Oversized inputs, unsafe paths, broken chains, unknown actors, stale subjects, invalid signatures, expiry and revocation leave required checks incomplete.

## Report and completion

The report retains the original envelope and public trust store as digested artifacts. Verified metadata identifies the signer, signed subject, run identities, replayed step count and final entity anchors with `agent_run_ids`. Raw file contents remain in the retained envelope.

External input bytes and accepted records' validity are checked again after command execution. Changed records/trust or expiration clear the affected verdict and block completion while preserving initial verification evidence. `verified` describes initial verification; `valid_at_completion` describes later validity. Use the check verdict and overall gate to decide delivery.

A complete valid ledger with a missing required declaration produces a violation (exit 1). Missing or unverifiable scope produces incomplete validation (exit 2). A passing selected rule establishes only that rule's declared scope.
