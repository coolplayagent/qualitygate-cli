# Git trailer declaration bindings

A `git_trailer` marker binds a test to the commit that last changed its parsed entity bytes. A declaration elsewhere in the comparison cannot satisfy that test. Git messages are declarations of claimed origin; they do not prove agent participation.

```yaml
schema_version: 1
rules:
  ai-code-traceability:
    parameters:
      languages: [java, python]
      provenance_scope: all_added_tests
      marker:
        type: git_trailer
        name: AI-Generated
        fields: [author, reason]
```

The matching commit message can contain this footer:

```text
Add order creation tests

AI-Generated: author='agent' reason='exercise order idempotency'
```

Custom test-method definitions use `binding.marker.type: git_trailer` and declare both `test_methods` and `commits` capabilities. Their existing change predicates remain in force. For `ai_only`, additionally configure [signed run records](provenance.md): the record establishes scope, while the associated commit supplies the declaration. Neither input substitutes for the other.

## Association

The CLI reads immutable trees and raw parent identities for the base/head ancestry. It compares parsed test entities between each commit and every parent. Exact entity bytes include the test declaration and attached annotations or decorators as defined by the language adapter.

| Change | Declaration source |
|---|---|
| New test or changed entity bytes | The commit introducing those final bytes |
| Unchanged test after an unrelated edit | Its existing associated commit |
| Committed file move with unchanged entity bytes | The matched predecessor's associated commit |
| New copy while the original remains | The copy's commit |
| Unchanged entity imported from a merge parent | The matching parent's associated commit |
| Identical final entity inherited independently from several parents | Every inherited associated commit must independently satisfy the marker and field contract |
| Conflict resolution producing different entity bytes | The merge commit |
| Uncommitted addition, entity edit or move in worktree/staged/path mode | No commit declaration; an old trailer cannot cover it |

Whitespace changes within the parsed entity count as byte changes. Line shifts outside the entity preserve association. Same-path symbols are reserved before cross-file name/body matches; ambiguous predecessors prevent completion. Deleting a test removes it from current validation targets.

A previously declared surviving test retains its marker obligation when changed, including when the new associated commit lacks a trailer. Custom dependency assertions also inspect unchanged tests with historical declarations, so a manifest-only edit cannot silently remove their required dependency.

The selected worktree or staged bytes determine whether an entity has an uncommitted change. Dirty working files cannot repair a staged snapshot. Explicit `--diff` and MR selections use the committed head, regardless of the local working tree. Path filtering controls rule targets; history association still covers the complete captured comparison.

## Trailer parsing and evidence

Parsing uses `git interpret-trailers --parse --no-divider` with `:` separators. This reads existing trailers without adding configured defaults or running configured trailer producers; folded values are unfolded. Footer names match case-insensitively. Text followed by a later prose paragraph does not become a valid trailer merely because it contains the marker name. These semantics follow the [Git documentation](https://git-scm.com/docs/git-interpret-trailers).

Each associated commit needs one nonempty trailer with the configured name. Duplicate matching trailers are ambiguous and leave the check incomplete. Required fields use the same nonempty assignment parser as source markers, independently for every inherited commit. Fields split between separate merge-parent declarations cannot satisfy one complete declaration.

Git replacement objects are disabled for snapshot operations. Raw commit parents are checked against the acquired ancestry, so truncated shallow history cannot masquerade as a root commit.

`metadata.git_trailers` retains the checked snapshot/comparison, base digest, commit IDs, parents, tree-content digests, original messages and message digests, parsed trailers, and current/baseline entity anchors. An entity's `commits` list contains null for an uncommitted change. `declaration_only: true` keeps this evidence distinct from authenticated agent-run provenance. The report provides the retained evidence; no separate signed acceptance is fabricated.

## Limits and incomplete checks

History is acquired once per check invocation when a selected rule uses this binding. File contents shared across commit trees are deduplicated in memory. Hashing and entity analysis run on blocking workers.

| Resource | Limit |
|---|---|
| Base/head ancestor union | 1,000 commits |
| Unique historical file bytes and commit/trailer text | 64 MiB |
| Total historical tree entries | 200,000 |
| Historical tree | Existing 20,000-file, 2 MiB-per-file and 12 MiB snapshot limits |
| Test entities | 50,000 per state and 200,000 across analyzed states |
| Retained declaration metadata | 8 MiB |
| Time | Cooperative 60-second acquisition and analysis budgets; existing Git, parser and structure-collection deadlines also apply |

All required historical objects must be available locally, and changed supported-language sources must parse at the analyzed checkpoints. Missing ancestry, unsupported historical Git entries, malformed syntax, ambiguous lineage and budget overruns report incomplete validation (exit 2). These limits can apply before any selected test matches. Obtain complete history within the documented limits and rerun the same scope; the CLI does not silently downgrade the binding.

With complete association, a missing declaration or required field is a violation (exit 1). A passing rule establishes only its configured scope. [Rust CLI fixtures](../tests/git_trailers.rs) cover unrelated commits, repair, staged/worktree differences, trailer parsing, moves/copies, merge resolution, shallow ancestry and replacement refs. [Adapter fixtures](../src/qualitygate/adapters/git_trailers_tests.rs) cover multi-parent field completeness, invalid facts, ambiguity and historical dependency obligations.
