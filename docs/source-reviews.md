# Normative source review records

A section hash establishes which specification text a rule refers to. A review record additionally records who examined the mapping, its external record reference and whether the rule was introduced, updated or still matches the revised section. A required source-bound rule stays incomplete until its selected policy includes a record bound to the current rule configuration. This also applies when no code entities match the rule.

## Configuration and migration

All custom definitions have a source, so each enabled custom rule needs an entry under `source_reviews`. Built-in rules need one when their policy setting has a `source`. Built-ins without a team source mapping retain their existing behavior. Earlier configurations containing only `source.content_hash` must add review records before source-bound rules can pass.

```yaml
source_reviews:
  descriptive-tests:
    id: source-review-42
    reviewer: testing-team
    reference: 'review-system/requests/42'
    resolution: rule_unchanged
    rationale: 'The clarified naming section still matches the existing assertion and examples.'
    binding_digest: 'sha256:<copy the expected binding digest after reviewing the rule>'
```

The example digest is a placeholder and is invalid until replaced. `resolution` is one of `initial_mapping`, `rule_updated` or `rule_unchanged`. It records the reviewer's conclusion; the CLI does not infer human review or correctness from code differences. Record IDs must be distinct within a policy. Review IDs, reviewers, references and rationales are required, bounded strings without control characters. Their maximum UTF-8 lengths are 128, 256, 2,048 and 4,096 bytes respectively. A policy permits at most 1,024 records within its existing 1 MiB input limit. Unknown fields, malformed digests, duplicate YAML keys, unknown rule keys and reviews without source-bound rules are invalid configuration.

The binding uses SHA-256 over a versioned canonical JSON object containing the rule ID, engine version, complete catalog entry and effective rule settings. The entry includes its definition origin, rule revision, capabilities, assertions and source. Overrides, enablement, severity, prerequisites and any additional source mapping participate. Review fields themselves are excluded to avoid a circular digest. The complete review inventory still participates in policy/rule digests, so prior external execution or acceptance records cannot be reused after a review-record change.

## Review workflow

1. Update the normative source mapping and executable assertions as needed. Add or update the positive and negative examples relevant to that change.
2. Run `qualitygate rules list --format json`. Inspect `source_reviews.<rule-id>.expected_binding_digest`, the definition and effective settings. Listing computes a candidate binding; it does not verify the referenced document or approve a mapping.
3. Have the team's review process examine the exact source, rule and examples. Record its identity, external reference, conclusion, rationale and binding digest in the candidate policy.
4. Run the check again. The source section must independently match its declared hash, and the record must match the effective rule binding. Retained violations still fail; valid review records cannot turn a failed rule into a pass.
5. Commit the reviewed policy and verification assets through the team's policy workflow. CI selects and protects the approved policy commit before invoking `--policy-ref`. A candidate record cannot override an older selected policy or authorize changes to its verification assets.

Changing only a source hash leaves the old review stale. Changing a rule version, definition, source binding or effective setting has the same effect. A review can conclude that assertions are unchanged, but must bind the newly reviewed mapping. The CLI never automatically refreshes an approval through `rules enable`, `init`, listing or a check.

## Evidence and trust

`policy.source_reviews` records the expected digest, supplied record and binding status (`missing`, `stale` or `bound`). Each selected source-bound rule also records `metadata.source_review`, including the policy source and trust label. `bound` establishes that the record matches the declared rule configuration; the actual document hash, source location and executable rule result are checked separately.

Candidate configurations remain `local_candidate`. A policy selected with `--policy-ref` remains `caller_supplied_ref`: the caller must establish that this reference is protected and approved. Reviewer names and record references are assertions stored in that policy. The CLI does not fetch the external reference, authenticate the named person or prove that a human performed the review. A local matching record is therefore not an independently authenticated approval. The trusted policy version is the authority for this contract. Signed manual acceptance and signed agent-run provenance have their separate protocols.

`policy.resolved_commit` records the selected reference's actual commit. Configuration, custom definitions and the requested [task contract](tasks.md) use this same immutable policy version, and diagnostic rechecks pin it explicitly.

Missing or stale records block required rules with exit 2 and retain existing rule diagnostics. Optional rules retain incomplete evidence without becoming mandatory; other usable checks can satisfy the selected gate. Staged checks use staged policies and document bytes, while `--policy-ref` selects the caller's policy version. Partial/quick plans retain their existing scope and pending-delivery semantics.

## Requirement-to-test evidence

| Requirement | Evidence |
|---|---|
| §6.6 record review instead of only refreshing hashes | `tests/source_reviews.rs::source_hash_refresh_alone_cannot_substitute_for_a_bound_review` |
| §6.6 selected policy authority and §3.5 staged isolation | `tests/source_reviews.rs::review_records_follow_selected_policy_and_staged_source_bytes` |
| §6.3 required/optional completeness and invalid configuration | `tests/source_reviews.rs::review_shape_keys_and_optional_rule_completeness_remain_explicit` |
| §8 stable review subject and inspectable record | `config/source_reviews.rs` binding fixtures and `domain/source_review.rs` pure contract tests |
| §6.6 unambiguous policy loading and source sections | Duplicate review/rule/parameter YAML tests; CommonMark source mapping fixtures in `application/policy_tests.rs` |
| §5 second-ecosystem and provenance composition | Reviewed temporary policies in custom-rule, Maven, Python, Git trailer and signed-provenance fixtures |

All test review assertions belong to isolated fixtures. They do not claim review of a real team's policy. Team review workflow deployment and measured pilot acceptance remain separate evidence.
