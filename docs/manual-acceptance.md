# Signed manual acceptance

A `manual` check consumes a signed decision from an external reviewer or review service. An authenticated approval passes that check; an authenticated rejection fails it. Missing, invalid, expired, revoked or mismatched records remain incomplete. A required dependent command stays blocked after rejection, so the combined gate can return 2 even though the review itself has a failing verdict.

```yaml
schema_version: 1
task_id: order-idempotency
acceptance:
  - id: reviewed-behavior
    description: Reviewer observed one order after duplicate requests
    verification:
      kind: manual
      check_id: behavior-review
      evidence_file: reviews/order-idempotency.json
    required: true
```

The same `kind` and `evidence_file` fields work in repository `checks`. The evidence filename is a normalized relative path under the external directory. It is not read from the checked snapshot. Manual checks cannot contain command arguments, tool probes, report mappings, project producers or command exit conditions.

```bash
qualitygate check --task task.yaml --policy-ref "$TRUSTED_COMMIT" \
  --trust-store /trusted/qualitygate/trust.json \
  --evidence-dir /trusted/qualitygate/records --format json
```

Both external paths must be supplied together and lie outside the checked repository, including ignored directories. The calling CI or harness controls and protects these paths, invocation arguments, trusted policy reference and signing keys. A caller-selected trust file does not establish the caller's authority by itself. Configured commands retain the caller's OS permissions; this feature supplies signature and input-integrity checks, not process isolation.

## Trust store

The store is strict JSON. Public keys are base64-encoded 32-byte Ed25519 keys; private keys are never accepted. This illustrative key placeholder must be replaced with the real review service's public key.

```json
{
  "schema_version": 1,
  "repository": "https://github.com/example/orders",
  "max_age_seconds": 86400,
  "keys": [{
    "id": "review-service",
    "public_key": "BASE64_PUBLIC_KEY",
    "checks": ["behavior-review"],
    "tasks": ["order-idempotency"],
    "allow_repository_checks": false
  }],
  "revoked_records": []
}
```

A signer must be authorized for the exact check ID and task ID. Repository checks separately require `allow_repository_checks: true`. Duplicate key material and weak keys are rejected. Removing a key revokes its authority; adding an ID to `revoked_records` revokes that record. The caller supplies current revocation state; the CLI does not contact a revocation service.

`repository` is an identity assigned by the trusted caller, not inferred from mutable Git remote configuration. Use a distinct identity and correctly scoped key authorization for each repository.

## Producing a record

Run the check with the trust store and initially empty evidence directory. Its blocked manual result contains `metadata.manual_acceptance.expected_subject`. The external reviewer independently establishes what was reviewed and copies the matching subject into its decision. A service must not approve an arbitrary subject merely because a client submitted it.

The signed payload is strict UTF-8 JSON with these fields:

| Field | Contract |
|---|---|
| `schema_version` | `1` |
| `record_id` | Unique service record ID, 1–256 bytes; used for revocation |
| `subject` | Exact `expected_subject`, including repository, snapshot, policy, check and optional task/acceptance IDs |
| `reviewer` | Nonempty reviewer identity asserted by the trusted signer, at most 1,024 bytes |
| `decision` | `approved` or `rejected` |
| `reason` | Nonempty observation or rejection reason, at most 16,384 bytes |
| `issued_at`, `expires_at` | UTC Unix seconds; `issued_at <= now < expires_at`, with positive validity no longer than the store's maximum age |

The subject binds snapshot mode, base/head commits, complete content digest, optional path filter and MR comparison identity. It also binds configuration, rule definitions/settings and task-contract digests. MR API response digests are excluded because transport payloads can change without changing the comparison. Code, contract, rule or comparison changes require a matching new record.

Sign the exact payload bytes using Ed25519 over DSSE pre-authentication encoding, with payload type `application/vnd.qualitygate.manual-acceptance.v1+json`. Store a DSSE JSON envelope containing `payloadType`, base64 `payload` and `signatures: [{"keyid": "review-service", "sig": "BASE64_SIGNATURE"}]` at the configured evidence filename. Standard and URL-safe base64, with or without padding, are accepted. The key ID only narrows candidate keys; verification uses the authorized public key and authenticated bytes.

This profile accepts 1–16 signatures and requires one valid authorized signer. It does not implement multi-party approval thresholds. Unknown fields, duplicate typed fields, malformed encodings, unsupported payload types and invalid signatures cannot establish acceptance. JSON payload bytes are authenticated directly without canonicalization or re-reading.

The Rust library exposes `qualitygate::adapters::attestation::{PAYLOAD_TYPE, pae}` and the typed `qualitygate::domain::ManualRecord` for producer integration. The [Rust CLI fixtures](../tests/manual.rs) demonstrate signing and the acceptance/repair sequence. Their fixed signing keys are test fixtures only.

The encoding follows the [DSSE protocol](https://github.com/secure-systems-lab/dsse/blob/master/protocol.md); verification uses [Ed25519 strict signature checks](https://docs.rs/ed25519-dalek/3.0.0/ed25519_dalek/struct.VerifyingKey.html#method.verify_strict). The review service remains responsible for the truth of its signed reviewer identity and observation.

## Evidence and completion

The report retains the bounded envelope and public trust store as digested artifacts, including a malformed envelope that could be read within budget. Successful verification records the signer key ID, public-key digest, signed decision and verification time. Manual checks do not fabricate a command, exit code or test execution count.

Inputs are captured before commands. Their bytes and accepted records' validity are checked again before publishing the gate. `verified` describes the initial signature/subject verification; `valid_at_completion` describes the later revalidation. A changed trust store/record or an expired approval prevents completion. Consumers use the check verdict and overall gate, rather than a metadata flag alone.

Budgets are 256 KiB per trust store, 64 trusted keys, 1 MiB per manual envelope, 128 selected record files and 8 MiB total external input shared with provenance records. Identifier lists are limited to 1,024 entries. Loading and signature verification have 30-second budgets; each manual check's `timeout_seconds` can further reduce its verification deadline. Non-regular files, symlinks, path traversal and size overruns are rejected.

[Agent-run provenance](provenance.md) and AI-only declaration scope use a separate signed payload and the trust store's `provenance_rules` authorization. A manual approval or manual signer authorization does not establish provenance. [Git trailer association](git-trailers.md) verifies commit declarations and does not establish manual acceptance or true agent participation.
