# Policy, task, and signed evidence

[简体中文](../../zh/02-reference/04-policy-task-and-signed-evidence.md) · [Volume index](README.md)

Repository policy, task acceptance, and external trust are distinct authorities.
The selected policy defines required checks and severities. A task contract adds
acceptance items for one request. A trust store and signed records authenticate
specific human or system declarations. None may silently weaken another.

## Policy lifecycle

A candidate revision records its parent, source inputs, immutable artifacts,
validation matrix, and observed differences. Independent paired validation uses
the same base/head Git revisions and permits before the candidate can be promoted.
Each policy captures its own exclusions; differing execution inputs bind both
snapshot identities instead of sharing one policy's filtered content.
Identical exclusion policies reuse immutable trees; differing exclusions execute
sequentially within a case to retain its two-tree memory reservation.
Approval and rollback are separate signed subjects. Active policy history is
append-only and auditable; a local branch name or digest alone is not approval.

The protected suite's optional `budget.snapshot_max_file_mib` defaults to 2 and
accepts integers 1–8. It applies equally to baseline/candidate captures and test
overlays. It is bound by the externally authorized suite digest, so capacity
changes require updated external authorization, not a candidate-policy edit.

Diagnostic trends, oracle review, gate execution, downstream benefit, and
maintenance cost are separate observations. Missing denominators and absent
measurements remain unknown. Policy evolution must not convert warnings or
incomplete evidence into pass merely to improve a metric.

## Task plans and rechecks

A task file contains a stable task ID, acceptance IDs, observable behavior,
verification kind, related check IDs, requiredness, and severity. Its digest is
reported with the selected policy and snapshot. Command verification retains
the exact invocation and proves the expected test/report was produced.

Recheck context binds the original report, selector, policy reference, task,
resource options, and evidence inputs. A changed final snapshot always requires
a new report. `quick`, path-scoped, or pending-delivery results cannot fulfill a
full acceptance item.

## Signed records and provenance

Manual acceptance and pilot authorization/acceptance use caller-owned DSSE and
Ed25519 trust inputs outside the checked repository. Verification checks exact
subject bytes, key scope, role, validity, maximum age, revocation, and binding
to snapshot/policy/task. Missing, stale, foreign, expired, revoked, or malformed
records are incomplete.

`snapshot.content_digest` identifies captured execution files. When present,
`snapshot.verification_digest` additionally binds scope, exclusions, changed
ranges and the path filter, and is required in signed snapshot bindings. The
same file contents do not make delivery and repository evidence interchangeable.
Reusable-core repository evaluations with a path filter also require this binding;
only repository evaluations without exclusions or a path filter may use the content
digest alone. Their same-scope CLI recheck argv is empty: the CLI only verifies
delivery. The separate delivery recheck explicitly requests delivery verification.
Feedback leaves unavailable or empty replay commands at `argv: null` and
`omitted: true`, while retaining an executable delivery recheck when available.

Agent provenance authenticates declared inputs, outputs, transformations, and
tool identity. Git trailers bind declarations to commits and reject ambiguous
or missing associations. These mechanisms establish provenance, not semantic
correctness or independent review.

Versioned decision and feedback envelopes preserve the original deterministic
payload plus evidence summaries, warnings, gaps, pending work, and omissions.
An optional external judgment provider may assess warnings in shadow or advisory
mode. Probabilistic, abstained, invalid, and execution-gap outcomes never change
the deterministic gate or approve policy.

---

[Previous: Projects and tool reports](03-projects-and-tool-reports.md) · [Contents](../README.md) · [Next: Exit codes and resource limits](05-exit-codes-and-resource-limits.md)

## Default document and media exclusions

`unstructured_documents` defaults to `enabled: true` and `retain: []`, including
existing policies with `exclude: []`. This changes acquisition coverage for those
policies. Newly initialized policies serialize the defaults explicitly. Runtimes
supporting this behavior advertise `snapshot.unstructured-documents.v1`.

```yaml
unstructured_documents:
  enabled: true
  retain: ['assets/required.pdf', 'tests/fixtures/**']
```

The version 1 classifier uses the final filename suffix, ignoring ASCII case:

- Documents: `pdf doc docx xls xlsx ppt pptx odt ods odp rtf`.
- Images: `png jpg jpeg gif webp bmp tif tiff ico avif heic heif`.
- Audio/video: `mp3 wav flac aac ogg m4a mp4 mov avi mkv webm mpeg mpg`.

It does not inspect contents or infer types from size. Markdown, text, HTML, SVG,
fonts, archives, unknown `.bin` files and other unlisted suffixes remain inputs.
A 9 MiB PDF can therefore be omitted while a 9 MiB `.bin` still makes acquisition
incomplete. This feature does not raise the 8 MiB per-file or 1 GiB total ceilings.

Defaults apply before content reads and byte budgets to both base and target,
including worktree, index, diff/MR and history. Excluded resources are absent from
command execution directories, receive no rule checks and have no content-integrity
claim. Commands that need a resource must retain it; missing command evidence
cannot establish a pass. Resource-only changes report an empty checked delivery.

`retain` uses the same bounded repository-relative, case-sensitive globs as
`exclude`. It restores matching default exclusions. Policy, task, rule source,
verification assets, provenance evidence and declared tool inputs are protected
from default exclusion. Explicit `exclude` still wins over `retain`; excluding a
protected input remains an error. Set `enabled: false` to disable only defaults.
Restored files remain subject to all normal read, type and capacity checks.

`selection.resource_exclusions` records classifier version, effective policy,
protected paths, and separate `default_excluded_*` / `explicit_excluded_*` counts
and path lists. The field names identify each exclusion reason. The verification
digest binds this evidence, including the complete excluded inventory. Excluded
content bytes are not hashed. Doctor budgets and init preflight expose the same
reason-separated evidence with bounded path details and complete counts; inspect
`details_truncated`. The existing `excluded_paths` remains the combined inventory.
