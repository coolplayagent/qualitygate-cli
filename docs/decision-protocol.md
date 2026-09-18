# Decision envelopes and optional judgment

The version 1 decision envelope is an opt-in JSON view of existing command
results. Add `--envelope --format json` to `check`, `check --feedback`,
`rules validate`, `selfcheck`, policy transitions, and pilot summaries. Export
the pinned schemas with `qualitygate schema decision`, `qualitygate schema
feedback`, and `qualitygate schema project-rule`; these commands emit JSON.
Existing command output and exit codes remain available without `--envelope`.

`command_kind` identifies the payload variant and the meaning of its exit code.
`check` retains the full report; `feedback` retains the bounded preview and its
digest-bearing pointer to the full report; rule validation and selfcheck retain
their own results. The envelope records snapshot, task, policy and evaluator
digests when those inputs exist. `decision_id` hashes the entire envelope except
the ID itself, including evidence pointers and verification limits. Consumers
must validate both the published schema and semantic bindings; unknown versions,
variants and fields fail closed. An incomplete command has `execution.complete:
false`, `gate.outcome: incomplete`, `gate.route: inspect_gap`, and exit 2. Warning
counts, execution gaps, pending delivery checks and preview omissions are
explicit. A complete pass with warnings, pending checks or optional execution
gaps routes to review. A pilot or policy transition outcome describes that command's result,
not a repository check.
For rule validation, `subject.source_digest` hashes the reported file/result
inventory; it is not a hash of every validated source file's bytes.

## External warning assessment

`qualitygate judgment run --report check.json --policy judgment.yaml
--output-dir judgments --format json` invokes a configured external provider.
The report must be a complete, digest-bound `check` report with a selected
warning finding. `--fingerprint` selects a warning when more than one exists.
The strict, at most 64 KiB judgment policy declares `schema_version: 1`,
`mode: shadow` or `advisory`, fixed `argv`, fixed `version_argv`, optional
confined `provider_inputs`, `timeout_seconds` from 1 to 120, a versioned
`question`, optional `calibration`, and an optional
`review_priority_threshold`. The provider receives a bounded JSON request on
stdin and must emit one tagged JSON `Assessment` on stdout. It can return
`deterministic`, `probabilistic`, `abstained`, or `execution_gap`. Unknown fields,
variants, nonfinite or nonnormalized distributions, changed evidence bindings,
missing tools and timeouts produce a typed execution gap. A successful
abstention is complete but offers no probability.

The question binds its ID, version, exact text, criteria, ordered choices,
positive choice and target event to `source_digest`. Create a question YAML
with these fields and `source_digest: ''`, then run `qualitygate judgment
question-digest --input question.yaml --format json`. Copy the reported digest
into the policy question. A changed question needs a new digest and calibration.
Probabilities denote the declared target event; severity, evidence completeness,
applicability, trust and review priority are separate fields. Probabilistic
output requires an in-domain calibration record with matching provider binding,
model, question, target event, repository, language, rule and time, plus a
declared independent label source digest. The provider
binding includes executable bytes, version output and declared input digests.
The repository scope digest binds the canonical local root path; another clone
requires its own calibration record. The request separately binds the check
snapshot and full report digests.
Neither `shadow` nor `advisory` changes the original gate, removes a finding,
or updates active policy. Advisory mode may prioritize human review.

Each run retains the full request, raw response, stderr, decision, policy and
report under the decision ID, with byte counts and SHA-256 digests. A later
provider invocation is a new observation; replay uses the retained inputs and
original response. The external provider and independent reviewers are outside
Qualitygate's trust boundary. Schema validity does not establish finding truth.

## Warning triage pilot

`qualitygate judgment pilot --runs judgments --labels labels.json --audit-seed
<at-least-16-byte-seed> --format json` summarizes at most 256 retained runs
against bounded independent labels. The labels file declares schema version,
repository, question, provider, model and calibrator digests, target event,
validation period, audit fraction and per-decision review labels. Calibration
training must end before validation begins. The summary reports Brier score,
log loss, reliability bins, risk versus coverage, threshold-near error,
rule/language slices, manual review count, abstentions and execution gaps.
A seeded sample of high-priority recommendations must have independent labels;
missing labels, changed scope or incomplete evidence yield exit 2. The pilot
never changes a repository gate. Reviewer identity and independence are
declared in the labels file; signature verification and policy approval remain
in the existing candidate, validation, external approval, promotion and
rollback workflow.
Accuracy and risk metrics describe the labeled cohort; the audit sample checks
that some high-priority recommendations receive independent review.
